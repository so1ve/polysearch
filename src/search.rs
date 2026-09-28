mod alignment;
mod matching;

use std::cmp::Ordering;

use matching::Rank;
use rapidhash::RapidHashMap;

use crate::config::Config;
use crate::correction::CorrectionIndex;
use crate::index::Index;
use crate::model::{Entry, EntryId, FieldId};
use crate::terms::Term;
use crate::text::NormalizedText;

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

    /// Searches by relevance, using `compare` to order equally strong matches.
    ///
    /// Built-in match quality takes precedence over `compare`. Length,
    /// position and entry ID break any remaining ties. Pass
    /// `|_, _| Ordering::Equal` to use only the built-in ordering.
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

        let text = NormalizedText::new(input);
        if text.is_empty() || text.len() > 512 {
            return Vec::new();
        }

        let original = Term { text, cost: 0 };
        let original_query_len = original.text.len();
        let mut best = RapidHashMap::default();

        self.collect_matches(&original, original_query_len, &mut best);

        // Dictionary correction runs only when no accepted match has zero
        // edits.
        let has_unedited_match = best.values().any(|(rank, _)| rank.quality.edits == 0);
        if !has_unedited_match
            && let Some(corrector) = &self.corrector
            && let Some((text, cost)) = corrector.correct(original.text.chars())
        {
            let corrected = Term {
                text: NormalizedText::new(text),
                cost,
            };
            self.collect_matches(&corrected, original_query_len, &mut best);
        }

        let mut results: Vec<_> = best.into_iter().collect();
        results.sort_by(|(left_id, (left, _)), (right_id, (right, _))| {
            left.quality
                .cmp(&right.quality)
                .then_with(|| compare(*left_id, *right_id))
                .then_with(|| left.tie_breaker.cmp(&right.tie_breaker))
                .then_with(|| left_id.cmp(right_id))
        });
        results.truncate(limit);

        results
            .into_iter()
            .map(|(entry, (_, field))| SearchResult { entry, field })
            .collect()
    }

    fn collect_matches(
        &self,
        query: &Term,
        original_query_len: usize,
        best: &mut RapidHashMap<EntryId, (Rank, FieldId)>,
    ) {
        let candidates = self
            .index
            .candidates(&query.text, self.config.max_candidates);

        for candidate in candidates {
            let Some(rank) =
                Rank::for_candidate(query, &candidate, original_query_len, &self.config)
            else {
                continue;
            };
            let term = candidate.term;

            let best = best.entry(term.entry).or_insert_with(|| (rank, term.field));

            if rank < best.0 {
                *best = (rank, term.field);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use crate::test_support::{entries, entry, result_ids};
    use crate::{Config, IDENTIFIER, PRIMARY_NAME, Searcher};

    #[test]
    fn custom_ordering_cannot_promote_weaker_matches() {
        let searcher = Searcher::new(
            entries(&["DingTalk", "GTK Demo", "GTK Widget Factory"]),
            Config::default(),
        );

        assert_eq!(result_ids(&searcher, "gtk", 10), [2, 3, 1]);

        let results = searcher.search("gtk", 10, |left, right| {
            let preferred = |id| match id {
                1 => 0,
                3 => 1,
                _ => 2,
            };

            preferred(left).cmp(&preferred(right))
        });

        assert_eq!(
            results
                .iter()
                .map(|result| result.entry)
                .collect::<Vec<_>>(),
            [3, 2, 1]
        );
        assert_eq!(
            searcher.search("gtk", 1, |left, right| right.cmp(&left))[0].entry,
            3
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
