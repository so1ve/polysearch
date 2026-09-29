use std::cmp::Ordering;

use rapidhash::RapidHashMap;

use crate::correction::CorrectionIndex;
use crate::index::Index;
use crate::matching::Rank;
use crate::text::NormalizedText;
use crate::{Config, Entry, EntryId, FieldId};

#[derive(Clone, Debug)]
pub struct SearchResult {
    pub entry: EntryId,
    pub field: FieldId,
}

pub struct Searcher {
    index: Index,
    corrector: Option<CorrectionIndex>,
    config: Config,
}

impl Searcher {
    /// Builds an index with token matching and bounded edit correction.
    ///
    /// # Panics
    ///
    /// Panics if entry IDs or field IDs are duplicated, or if more than 2^32
    /// indexed terms are generated. Field IDs are unique across the entire
    /// index, including fields belonging to different entries.
    #[must_use]
    pub fn new(entries: impl IntoIterator<Item = Entry>, config: Config) -> Self {
        let index = Index::new(entries);
        let corrector = (config.enable_correction && config.max_edits > 0)
            .then(|| CorrectionIndex::new(&index.vocabulary(), config.max_edits));

        Self {
            index,
            corrector,
            config,
        }
    }

    /// Orders results by field priority, then exact matches, completions,
    /// and fuzzy matches.
    ///
    /// For the same priority and tier, `compare` takes precedence over built-in
    /// match details. Return `Ordering::Equal` to use only the built-in
    /// ordering.
    #[must_use]
    pub fn search(
        &self,
        input: &str,
        limit: usize,
        mut compare: impl FnMut(EntryId, EntryId) -> Ordering,
    ) -> Vec<SearchResult> {
        if limit == 0 || self.config.max_candidates == 0 {
            return Vec::new();
        }

        let query = NormalizedText::new(input);
        let query_len = query.chars.len();

        if query_len == 0 || query_len > 512 {
            return Vec::new();
        }

        let mut best = RapidHashMap::default();

        self.collect_matches(&query, query_len, 0, &mut best);

        // Dictionary correction runs only when no accepted match has zero
        // edits.
        let has_unedited_match = best.values().any(|hit| hit.rank.edits == 0);
        if !has_unedited_match
            && let Some(corrector) = &self.corrector
            && let Some((text, edits)) = corrector.correct(&query.chars)
        {
            let corrected = NormalizedText::new(text);
            self.collect_matches(&corrected, query_len, edits, &mut best);
        }

        let mut results: Vec<_> = best.into_iter().collect();
        results.sort_by(|(left_id, left), (right_id, right)| {
            left.rank
                .priority
                .cmp(&right.rank.priority)
                .then_with(|| left.rank.tier.cmp(&right.rank.tier))
                .then_with(|| compare(*left_id, *right_id))
                .then_with(|| left.rank.cmp(&right.rank))
                .then_with(|| left_id.cmp(right_id))
        });
        results.truncate(limit);

        results
            .into_iter()
            .map(|(entry, hit)| SearchResult {
                entry,
                field: hit.field,
            })
            .collect()
    }

    fn collect_matches(
        &self,
        query: &NormalizedText,
        original_query_len: usize,
        correction_edits: u16,
        best: &mut RapidHashMap<EntryId, Hit>,
    ) {
        let candidates = self.index.candidates(query, self.config.max_candidates);

        for candidate in candidates {
            let Some(rank) = Rank::for_candidate(
                &query.chars,
                &candidate,
                original_query_len,
                correction_edits,
                self.config.max_edits,
            ) else {
                continue;
            };
            let term = candidate.term;

            let best = best.entry(term.entry).or_insert(Hit {
                rank,
                field: term.field,
            });

            if rank < best.rank {
                *best = Hit {
                    rank,
                    field: term.field,
                };
            }
        }
    }
}

struct Hit {
    rank: Rank,
    field: FieldId,
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use crate::test_support::{entries, entry, result_ids};
    use crate::{ALIAS, Config, IDENTIFIER, KEYWORD, PRIMARY_NAME, Searcher};

    #[test]
    fn history_respects_field_priority_and_match_tier() {
        let searcher = Searcher::new(
            [
                entry(1, vec![(1, PRIMARY_NAME, "GTK")]),
                entry(2, vec![(2, KEYWORD, "GTK Demo")]),
                entry(3, vec![(3, PRIMARY_NAME, "GTKraken")]),
                entry(4, vec![(4, ALIAS, "Xgtkview")]),
                entry(5, vec![(5, PRIMARY_NAME, "General Toolkit")]),
                entry(6, vec![(6, PRIMARY_NAME, "DingTalk")]),
                entry(7, vec![(7, PRIMARY_NAME, "GTL")]),
                entry(8, vec![(8, PRIMARY_NAME, "GetKit Tools")]),
            ],
            Config::default(),
        );

        assert_eq!(result_ids(&searcher, "gtk", 10), [1, 3, 5, 8, 6, 7, 4, 2]);

        // History only reorders matches with the same priority and tier.
        let results = searcher.search("gtk", 10, |left, right| right.cmp(&left));

        assert_eq!(
            results
                .iter()
                .map(|result| result.entry)
                .collect::<Vec<_>>(),
            [1, 5, 3, 8, 7, 6, 4, 2]
        );
        assert_eq!(
            searcher.search("gtk", 1, |left, right| right.cmp(&left))[0].entry,
            1
        );
    }

    #[test]
    fn title_prefix_outranks_exact_metadata_and_remains_the_best_field() {
        let searcher = Searcher::new(
            [
                entry(
                    1,
                    vec![
                        (1, PRIMARY_NAME, "ChatGPT Community"),
                        (2, KEYWORD, "A chat client"),
                    ],
                ),
                entry(
                    2,
                    vec![
                        (3, PRIMARY_NAME, "Kelivo"),
                        (4, KEYWORD, "A Flutter LLM chat client"),
                    ],
                ),
                entry(
                    3,
                    vec![(5, PRIMARY_NAME, "Thunderbird"), (6, KEYWORD, "Email Chat")],
                ),
            ],
            Config::default(),
        );

        // Favor Thunderbird and Kelivo in history; ChatGPT must still lead.
        let results = searcher.search("chat", 3, |left, right| right.cmp(&left));

        assert_eq!(
            results
                .iter()
                .map(|result| (result.entry, result.field))
                .collect::<Vec<_>>(),
            [(1, 1), (3, 6), (2, 4)]
        );
        assert_eq!(
            searcher.search("chat", 1, |left, right| right.cmp(&left))[0].entry,
            1
        );
    }

    #[test]
    fn correction_and_direct_alignment_use_the_same_edit_cost() {
        let searcher = Searcher::new(entries(&["woixon", "Messenger Weixni"]), Config::default());

        assert_eq!(result_ids(&searcher, "weixin", 5), [2, 1]);
    }

    #[test]
    fn corrections_do_not_expand_an_exact_result() {
        let searcher = Searcher::new(
            [
                entry(1, vec![(1, IDENTIFIER, "weixni")]),
                entry(2, vec![(2, IDENTIFIER, "weixin")]),
            ],
            Config::default(),
        );

        assert_eq!(result_ids(&searcher, "weixni", 5), [1]);
    }

    #[test]
    fn default_search_only_limits_results_at_the_callers_request() {
        let searcher = Searcher::new(
            (1..=600).map(|id| entry(u64::from(id), vec![(id, PRIMARY_NAME, "xapp")])),
            Config::default(),
        );

        for query in ["xapp", "app"] {
            for limit in [0, 1, 20, 600, 1_000] {
                assert_eq!(
                    searcher.search(query, limit, |_, _| Ordering::Equal).len(),
                    limit.min(600)
                );
            }
        }
        assert!(
            searcher
                .search(" /-_ ", 10, |_, _| Ordering::Equal)
                .is_empty()
        );
    }

    #[test]
    fn zero_edits_disables_direct_fuzzy_and_dictionary_correction() {
        for role in [PRIMARY_NAME, IDENTIFIER] {
            let searcher = Searcher::new(
                [entry(1, vec![(1, role, "weixin")])],
                Config {
                    max_edits: 0,
                    ..Config::default()
                },
            );

            assert!(
                searcher
                    .search("weixni", 5, |_, _| Ordering::Equal)
                    .is_empty()
            );
            assert_eq!(result_ids(&searcher, "weixin", 5), [1]);
        }
    }
}
