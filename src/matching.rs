mod alignment;
mod subsequence;

use alignment::{AlignmentKind, align};

use crate::index::{Candidate, TokenMatch};

// Field order supplies tie-breakers after the tier and caller preference.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct Rank {
    pub tier: Tier,
    pub edits: u16,
    role: u8,
    kind: MatchKind,
    penalty: u16,
    gaps: u16,
    unmatched: u16,
    start: u16,
    frequency: u16,
}

impl Rank {
    pub fn for_candidate(
        query: &[char],
        candidate: &Candidate<'_>,
        original_query_len: usize,
        correction_edits: u16,
        max_edits: u16,
    ) -> Option<Self> {
        let term = candidate.term;
        let role = term.role;

        if original_query_len.min(query.len()) < usize::from(role.min_query_chars) {
            return None;
        }

        // A corrected query must match a whole term or token; a guessed word
        // must not introduce new prefix completions.
        if correction_edits > 0 {
            if !role.allow_correction {
                return None;
            }

            if term.chars.as_ref() != query && candidate.token != Some(TokenMatch::Exact) {
                return None;
            }
        }

        let alignment = align(
            query,
            &term.chars,
            &term.token_starts,
            max_edits - correction_edits,
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
            AlignmentKind::Abbreviation { initials_only } => {
                if !role.allow_substring || (original_query_len < 3 && !initials_only) {
                    return None;
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
                if !role.allow_substring {
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

        let edits = correction_edits + alignment.edits;
        let tier = if edits > 0 || kind == MatchKind::Subsequence {
            Tier::Fuzzy
        } else if kind == MatchKind::Exact {
            Tier::Exact
        } else {
            Tier::Completion
        };

        Some(Self {
            tier,
            edits,
            role: role.priority,
            kind,
            penalty: term.penalty,
            gaps: alignment.gaps,
            unmatched: alignment.unmatched,
            start: alignment.start,
            frequency: term.frequency.min(u32::from(u16::MAX)) as u16,
        })
    }
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub enum Tier {
    Exact,
    Completion,
    Fuzzy,
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
    fn exact_matches_outrank_completions_and_subsequences() {
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

        assert_eq!(result_ids(&searcher, "gtk", 10), [2, 4, 3, 1]);
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
