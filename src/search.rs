mod alignment;
mod matching;

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

    #[must_use]
    pub fn search(&self, input: &str, limit: usize) -> Vec<SearchResult> {
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

        // A correction is a fallback for a weak miss, never a way to fill
        // remaining result slots after a zero-edit match.
        let has_strong_match = best.values().any(|(rank, _)| rank.is_strong());
        if !has_strong_match
            && let Some(corrector) = &self.corrector
            && let Some((text, cost)) = corrector.correct(original.text.chars())
        {
            let corrected = Term {
                text: NormalizedText::new(text),
                cost,
            };
            self.collect_matches(&corrected, original_query_len, &mut best);
        }

        let mut results: Vec<_> = best.into_values().collect();
        results.sort_by(|(left, _), (right, _)| left.cmp(right));
        results.truncate(limit);

        results.into_iter().map(|(_, result)| result).collect()
    }

    fn collect_matches(
        &self,
        query: &Term,
        original_query_len: usize,
        best: &mut RapidHashMap<EntryId, (Rank, SearchResult)>,
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

            if best
                .get(&term.entry)
                .is_some_and(|(previous, _)| previous <= &rank)
            {
                continue;
            }

            best.insert(
                term.entry,
                (
                    rank,
                    SearchResult {
                        entry: term.entry,
                        field: term.field,
                    },
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::{entries, entry, result_ids};
    use crate::{Config, IDENTIFIER, PRIMARY_NAME, Searcher};

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
                assert_eq!(searcher.search(query, limit).len(), limit.min(600));
            }
        }
        assert!(searcher.search(" /-_ ", 10).is_empty());
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

            assert!(searcher.search("weixni", 5).is_empty());
            assert_eq!(result_ids(&searcher, "weixin", 5), [1]);
        }
    }
}
