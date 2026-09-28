use super::alignment::{AlignmentKind, align};
use crate::config::Config;
use crate::index::{Candidate, TokenMatch};
use crate::terms::Term;

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct Rank {
    pub quality: Quality,
    pub tie_breaker: TieBreaker,
}

impl Rank {
    pub fn for_candidate(
        query: &Term,
        candidate: &Candidate<'_>,
        original_query_len: usize,
        config: &Config,
    ) -> Option<Self> {
        let term = candidate.term;
        let role = term.role;

        if original_query_len.min(query.text.len()) < usize::from(role.min_query_chars) {
            return None;
        }

        // Corrected queries carry a positive cost and may match only a whole
        // term or token. They must not turn a guessed word into a new prefix.
        if query.cost > 0 {
            if !role.allow_correction {
                return None;
            }

            if term.chars.as_ref() != query.text.chars()
                && candidate.token != Some(TokenMatch::Exact)
            {
                return None;
            }
        }

        let alignment = align(
            query.text.chars(),
            &term.chars,
            &term.token_starts,
            config.max_edits - query.cost,
        )?;

        let kind = match alignment.kind {
            AlignmentKind::Contiguous => {
                if alignment.start == 0 || candidate.token.is_some() {
                    MatchKind::Prefix
                } else if role.allow_substring {
                    MatchKind::Substring
                } else {
                    return None;
                }
            }
            AlignmentKind::Abbreviation {
                crosses_token_boundary,
                initials_only,
            } => {
                if !role.allow_substring {
                    return None;
                }

                if original_query_len < 3 {
                    let acronym = crosses_token_boundary && initials_only;
                    let near_start =
                        !crosses_token_boundary && alignment.start == 0 && alignment.gaps <= 1;

                    if !acronym && !near_start {
                        return None;
                    }
                }

                MatchKind::Abbreviation
            }
            AlignmentKind::Subsequence => {
                if !role.allow_substring
                    || (original_query_len < 3 && (alignment.start > 0 || alignment.gaps > 1))
                {
                    return None;
                }

                MatchKind::Subsequence
            }
            AlignmentKind::Fuzzy => {
                if !role.allow_substring || !role.allow_fuzzy || query.text.len() < 3 {
                    return None;
                }

                MatchKind::Prefix
            }
        };

        let kind = if candidate.token == Some(TokenMatch::Exact)
            || (alignment.gaps == 0 && alignment.unmatched == 0)
        {
            MatchKind::Exact
        } else {
            kind
        };

        Some(Self {
            quality: Quality {
                edits: query.cost + alignment.edits,
                loose: kind == MatchKind::Subsequence,
                role: role.priority,
                kind,
                cost: term.cost,
            },
            tie_breaker: TieBreaker {
                gaps: alignment.gaps,
                unmatched: alignment.unmatched,
                start: alignment.start,
                frequency: term.frequency.min(u32::from(u16::MAX)) as u16,
            },
        })
    }
}

// Field order defines relevance: spelling, structured matches, field role,
// match kind, then representation cost. History cannot override these.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct Quality {
    pub edits: u16,
    loose: bool,
    role: u8,
    kind: MatchKind,
    cost: u16,
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct TieBreaker {
    gaps: u16,
    unmatched: u16,
    start: u16,
    frequency: u16,
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum MatchKind {
    Exact,
    Prefix,
    Abbreviation,
    Substring,
    Subsequence,
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use crate::test_support::{entries, entry, result_ids};
    use crate::{Config, IDENTIFIER, KEYWORD, PRIMARY_NAME, Searcher};

    #[test]
    fn direct_matches_outrank_incidental_name_subsequences() {
        let searcher = Searcher::new(
            [
                entry(1, vec![(1, PRIMARY_NAME, "DingTalk")]),
                entry(2, vec![(2, PRIMARY_NAME, "GTK Demo")]),
                entry(3, vec![(3, PRIMARY_NAME, "GTKraken")]),
                entry(
                    4,
                    vec![(4, KEYWORD, "A simple editor demonstrating GTK printing")],
                ),
            ],
            Config::default(),
        );

        assert_eq!(result_ids(&searcher, "gtk", 10), [2, 3, 4, 1]);
    }

    #[test]
    fn identifier_tokens_are_accepted_but_inside_token_substrings_are_rejected() {
        let searcher = Searcher::new(
            [
                entry(1, vec![(1, IDENTIFIER, "org.gnome.Nautilus")]),
                entry(2, vec![(2, IDENTIFIER, "xfoo foo")]),
            ],
            Config {
                enable_correction: false,
                ..Config::default()
            },
        );

        assert_eq!(result_ids(&searcher, "nautilus", 5), [1]);
        assert!(
            searcher
                .search("autilus", 5, |_, _| Ordering::Equal)
                .is_empty()
        );
        assert_eq!(result_ids(&searcher, "foo", 5), [2]);
    }

    #[test]
    fn names_rank_by_full_length_before_identifiers() {
        let searcher = Searcher::new(
            [
                entry(1, vec![(1, PRIMARY_NAME, "Code Editor")]),
                entry(2, vec![(2, IDENTIFIER, "editor")]),
                entry(3, vec![(3, PRIMARY_NAME, "Editor")]),
            ],
            Config::default(),
        );

        assert_eq!(result_ids(&searcher, "editor", 5), [3, 1, 2]);
        assert_eq!(result_ids(&searcher, "e", 5), [3, 1]);
    }

    #[test]
    fn fuzzy_prefixes_keep_exact_matches_first_and_respect_edit_limits() {
        for (max_edits, expected) in [(0, vec![]), (1, vec![2]), (2, vec![2, 1])] {
            let searcher = Searcher::new(
                entries(&["algermusicplayer", "algormusicplayer"]),
                Config {
                    enable_correction: false,
                    max_edits,
                    ..Config::default()
                },
            );

            assert_eq!(
                searcher.search("algormusic", 5, |_, _| Ordering::Equal)[0].entry,
                2
            );
            assert_eq!(
                result_ids(&searcher, "algormusix", 5),
                expected,
                "max_edits={max_edits}"
            );
        }
    }

    #[test]
    fn complete_typo_matches_rank_before_equivalent_prefix_completions() {
        let searcher = Searcher::new(entries(&["amdbuild", "amdbase", "amdb"]), Config::default());

        assert_eq!(result_ids(&searcher, "amdm", 5), [3, 2, 1]);
    }

    #[test]
    fn spelling_quality_ranks_before_name_frequency() {
        let searcher = Searcher::new(
            entries(&["algermusicplayer", "algomusicplayer", "algomusicplayer"]),
            Config::default(),
        );

        assert_eq!(result_ids(&searcher, "algo", 5), [2, 3, 1]);
    }

    #[test]
    fn fuzzy_ranking_uses_the_consumed_prefix_length() {
        let searcher = Searcher::new(
            entries(&["abcefg", "abcyef", "abcef"]),
            Config {
                enable_correction: false,
                ..Config::default()
            },
        );

        assert_eq!(result_ids(&searcher, "abcxef", 5), [2, 3, 1]);
    }

    #[test]
    fn corrections_do_not_use_broad_keyword_fields() {
        let searcher = Searcher::new(
            [entry(
                1,
                vec![
                    (1, PRIMARY_NAME, "KDE Connect SMS"),
                    (2, KEYWORD, "Read and send SMS messages"),
                ],
            )],
            Config::default(),
        );

        assert!(searcher.search("amd", 5, |_, _| Ordering::Equal).is_empty());
    }

    #[test]
    fn short_substrings_and_near_pairs_do_not_allow_arbitrary_subsequences() {
        let searcher = Searcher::new(entries(&["zed", "oopz"]), Config::default());

        assert_eq!(result_ids(&searcher, "zd", 5), [1]);
        assert_eq!(result_ids(&searcher, "op", 5), [2]);
        assert!(searcher.search("oz", 5, |_, _| Ordering::Equal).is_empty());
    }
}
