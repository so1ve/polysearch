use super::alignment::align;
use crate::config::Config;
use crate::index::{Candidate, TokenMatch};
use crate::model::EntryId;
use crate::terms::Term;

/// An accepted match's sorting key
#[derive(Eq, Ord, PartialEq, PartialOrd)]
pub struct Rank {
    role_penalty: u16,
    // intentionally put before candidate_cost to prioritize fewer edits over cheaper
    // representations (when comparing Ord)
    edit_cost: u16,
    candidate_cost: u16,
    gap_cost: u16,
    start: u16,
    frequency_penalty: u16,
    stable_id: EntryId,
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

        if alignment.crosses_token_boundary() && !alignment.token_progression() {
            return None;
        }

        let contiguous = alignment.is_contiguous();
        let substring = alignment.start > 0 && candidate.token.is_none();
        let fuzzy = alignment.edits > 0;

        if original_query_len < 3 && !contiguous {
            let acronym = alignment.crosses_token_boundary()
                && alignment.token_starts()
                && alignment.token_progression();
            let near_subsequence =
                !alignment.crosses_token_boundary() && alignment.start == 0 && alignment.gaps <= 1;

            if !acronym && !near_subsequence {
                return None;
            }
        }

        if (substring || !contiguous) && !role.allow_substring {
            return None;
        }

        if fuzzy && (!role.allow_fuzzy || query.text.len() < 3) {
            return None;
        }

        Some(Self {
            role_penalty: u16::from(role.priority),
            edit_cost: query.cost + alignment.edits,
            candidate_cost: term.cost,
            gap_cost: alignment.gaps,
            start: alignment.start,
            frequency_penalty: term.frequency.min(u32::from(u16::MAX)) as u16,
            stable_id: term.entry,
        })
    }

    pub const fn is_strong(&self) -> bool {
        self.edit_cost == 0
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::{entries, entry, result_ids};
    use crate::{Config, IDENTIFIER, KEYWORD, PRIMARY_NAME, Searcher};

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
        assert!(searcher.search("autilus", 5).is_empty());
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

            assert_eq!(searcher.search("algormusic", 5)[0].entry, 2);
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

        assert!(searcher.search("amd", 5).is_empty());
    }

    #[test]
    fn short_substrings_and_near_pairs_do_not_allow_arbitrary_subsequences() {
        let searcher = Searcher::new(entries(&["zed", "oopz"]), Config::default());

        assert_eq!(result_ids(&searcher, "zd", 5), [1]);
        assert_eq!(result_ids(&searcher, "op", 5), [2]);
        assert!(searcher.search("oz", 5).is_empty());
    }
}
