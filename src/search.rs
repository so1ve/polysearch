use std::cmp::Reverse;

use crate::index::Index;
use crate::text::normalize;
use crate::{Config, Entry, EntryId, FieldId, matching};

#[derive(Clone, Debug)]
pub struct SearchResult {
    pub entry: EntryId,
    pub field: FieldId,
}

pub struct Searcher {
    index: Index,
    config: Config,
}

impl Searcher {
    /// Prepares shared text variants for Frizbee matching.
    ///
    /// # Panics
    ///
    /// Panics if entry IDs or field IDs are duplicated. Field IDs must be
    /// unique across the entire index, including fields belonging to
    /// different entries.
    #[must_use]
    pub fn new(entries: impl IntoIterator<Item = Entry>, config: Config) -> Self {
        Self {
            index: Index::new(entries),
            config,
        }
    }

    /// Ranks exact names and aliases first, then matches by quality, field,
    /// and spelling confidence.
    ///
    /// `preference` returns 0–255 for each matching entry, adding up to 50
    /// points to a score where a perfect completion is worth 1000. Return
    /// zero for built-in ordering alone. The callback runs once per
    /// matching entry.
    #[must_use]
    pub fn search(
        &self,
        input: &str,
        limit: usize,
        mut preference: impl FnMut(EntryId) -> u8,
    ) -> Vec<SearchResult> {
        if limit == 0 {
            return Vec::new();
        }

        let query = normalize(input);

        if query.is_empty() {
            return Vec::new();
        }

        let mut best = vec![None; self.index.entries.len()];

        for (source, rank) in matching::matches(&query, &self.index.spellings, &self.config) {
            let candidate = (rank, Reverse(source.field));

            if best[source.entry].is_none_or(|previous| candidate > previous) {
                best[source.entry] = Some(candidate);
            }
        }

        let mut results: Vec<_> = best
            .into_iter()
            .enumerate()
            .filter_map(|(index, hit)| {
                hit.map(|(mut rank, Reverse(field))| {
                    let entry = self.index.entries[index];
                    rank.score += i32::from(preference(entry)) * 50 / 255;

                    (Reverse(rank), entry, field)
                })
            })
            .collect();

        results.sort_unstable();
        results.truncate(limit);

        results
            .into_iter()
            .map(|(_, entry, field)| SearchResult { entry, field })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use frizbee::{CaseMatching, Config as MatcherConfig, Matcher};

    use crate::test_support::{entries, entry, result_ids};
    use crate::{ALIAS, Config, IDENTIFIER, KEYWORD, PRIMARY_NAME, Searcher};

    #[test]
    fn strong_metadata_matches_outrank_weak_names_even_with_history() {
        let searcher = Searcher::new(
            [
                entry(1, vec![(1, PRIMARY_NAME, "GTK Demo")]),
                entry(2, vec![(2, PRIMARY_NAME, "GTKraken")]),
                entry(3, vec![(3, PRIMARY_NAME, "DingTalk")]),
                entry(
                    4,
                    vec![(4, KEYWORD, "An editor demonstrating GTK printing")],
                ),
            ],
            Config::default(),
        );
        let results = searcher.search("gtk", 10, |id| if id == 3 { 255 } else { 0 });

        assert_eq!(
            results
                .iter()
                .map(|result| result.entry)
                .collect::<Vec<_>>(),
            [1, 2, 4, 3]
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
        let results = searcher.search("chat", 3, |id| if id == 3 { 255 } else { 0 });

        assert_eq!(
            results
                .iter()
                .map(|result| (result.entry, result.field))
                .collect::<Vec<_>>(),
            [(1, 1), (3, 6), (2, 4)]
        );
    }

    #[test]
    fn shared_spellings_keep_unique_results_and_the_best_field() {
        let searcher = Searcher::new(
            [
                entry(1, vec![(1, ALIAS, "Editor"), (2, PRIMARY_NAME, "Editor")]),
                entry(2, vec![(3, PRIMARY_NAME, "Editor")]),
            ],
            Config::default(),
        );
        let results = searcher.search("editor", 10, |_| 0);

        assert_eq!(
            results
                .iter()
                .map(|result| (result.entry, result.field))
                .collect::<Vec<_>>(),
            [(1, 2), (2, 3)]
        );
    }

    #[test]
    fn search_only_limits_results_at_the_callers_request() {
        let searcher = Searcher::new(
            (1..=600).map(|id| entry(u64::from(id), vec![(id, PRIMARY_NAME, "xapp")])),
            Config::default(),
        );

        for limit in [0, 20, usize::MAX] {
            assert_eq!(searcher.search("app", limit, |_| 0).len(), limit.min(600));
        }

        assert!(searcher.search(" /-_ ", 10, |_| 0).is_empty());
    }

    #[test]
    fn ranking_keeps_all_frizbee_matches() {
        let names = [
            "gtk",
            "dingtalk",
            "gnometweaks",
            "toolkit",
            "zed",
            "gnometexteditor",
            "firefox",
            "fire",
        ];
        let searcher = Searcher::new(entries(&names), Config::default());
        let config = MatcherConfig {
            max_typos: Some(1),
            casing: CaseMatching::Ignore,
            ..MatcherConfig::default()
        };

        for query in ["gtk", "zd", "fier"] {
            let mut expected: Vec<_> = Matcher::new(query, &config)
                .match_list(&names)
                .into_iter()
                .map(|hit| u64::from(hit.index) + 1)
                .collect();
            let mut actual = result_ids(&searcher, query, names.len());
            expected.sort_unstable();
            actual.sort_unstable();

            assert_eq!(actual, expected, "query={query}");
        }
    }

    #[test]
    fn long_queries_and_fields_remain_searchable() {
        let query = "abcd".repeat(129);
        let text = format!("{} {query}", "x".repeat(4_096));
        let searcher = Searcher::new(entries(&[&text]), Config::default());

        for query in [query.as_str(), "abcd"] {
            assert_eq!(result_ids(&searcher, query, 1), [1]);
        }
    }

    #[test]
    fn zero_typos_keeps_subsequences_and_identifier_tokens() {
        let searcher = Searcher::new(
            [
                entry(1, vec![(1, PRIMARY_NAME, "Zed")]),
                entry(2, vec![(2, IDENTIFIER, "org.gnome.Nautilus")]),
            ],
            Config { max_typos: 0 },
        );

        assert_eq!(result_ids(&searcher, "zd", 5), [1]);
        assert_eq!(result_ids(&searcher, "nautilus", 5), [2]);
        assert!(result_ids(&searcher, "zedd", 5).is_empty());
    }

    #[test]
    fn preferences_reorder_close_matches_but_not_exact_names_or_aliases() {
        let searcher = Searcher::new(
            [
                entry(1, vec![(1, PRIMARY_NAME, "Calendar")]),
                entry(2, vec![(2, PRIMARY_NAME, "Calculator")]),
                entry(3, vec![(3, PRIMARY_NAME, "Calibre")]),
                entry(4, vec![(4, ALIAS, "Cal")]),
            ],
            Config::default(),
        );
        let mut scored = Vec::new();
        let results = searcher.search("cal", 4, |id| {
            scored.push(id);

            if id == 3 { 255 } else { 0 }
        });

        assert_eq!(
            results
                .iter()
                .map(|result| result.entry)
                .collect::<Vec<_>>(),
            [4, 3, 1, 2]
        );
        scored.sort_unstable();

        assert_eq!(scored, [1, 2, 3, 4]);
    }

    #[test]
    fn spaced_queries_match_all_words_in_any_order() {
        let searcher = Searcher::new(
            entries(&["VisualStudioCode", "VisualStudio", "VideoConverter"]),
            Config::default(),
        );

        for query in ["studio code", "code studio", "stduio cdoe", "vs code"] {
            assert_eq!(result_ids(&searcher, query, 1), [1], "query={query}");
        }

        assert!(result_ids(&searcher, "studio zzz", 3).is_empty());
    }

    #[test]
    fn short_misspelled_names_outrank_fragments_in_long_names() {
        let searcher = Searcher::new(
            entries(&[
                "fingerprint",
                "file folder",
                "fire",
                "Wireshark",
                "LibreOffice Writer",
                "Fire Engine",
                "Fire Extinguisher",
                "Crossed Fingers",
            ]),
            Config::default(),
        );

        assert_eq!(result_ids(&searcher, "fier", 1), [3]);
        assert_eq!(result_ids(&searcher, "wirter", 1), [5]);
    }
}
