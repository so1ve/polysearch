mod subsequence;

use crate::distance::{MatchLength, damerau_levenshtein};

pub struct Alignment {
    pub edits: u16,
    pub gaps: u16,
    pub unmatched: u16,
    pub start: u16,
    pub kind: AlignmentKind,
}

pub enum AlignmentKind {
    Contiguous,
    Subsequence,
    Abbreviation {
        crosses_token_boundary: bool,
        initials_only: bool,
    },
    Fuzzy,
}

#[must_use]
pub fn align(
    query: &[char],
    candidate: &[char],
    token_starts: &[u16],
    max_edits: u16,
) -> Option<Alignment> {
    let query_len = query.len();

    if let Some(start) = candidate.windows(query_len).position(|part| part == query) {
        let contiguous = Alignment {
            edits: 0,
            gaps: 0,
            unmatched: (candidate.len() - query_len) as u16,
            start: start as u16,
            kind: AlignmentKind::Contiguous,
        };

        // A later token-initial path can be more meaningful than an earlier
        // incidental substring (`ce` in `Office Code Editor`). Only inspect
        // that alternate path when the term actually has multiple tokens;
        // single-token substrings keep their cheap direct alignment.
        if start == 0 || token_starts.len() <= 1 {
            return Some(contiguous);
        }

        if let Some(subsequence) = subsequence::align(query, candidate, token_starts)
            && matches!(
                subsequence.kind,
                AlignmentKind::Abbreviation {
                    crosses_token_boundary: true,
                    ..
                }
            )
        {
            return Some(subsequence);
        }

        return Some(contiguous);
    }

    if let Some(alignment) = subsequence::align(query, candidate, token_starts) {
        return Some(alignment);
    }

    let (edits, end) = damerau_levenshtein(query, candidate, max_edits, MatchLength::Prefix)?;

    Some(Alignment {
        edits,
        gaps: 0,
        unmatched: (candidate.len() - end) as u16,
        start: 0,
        kind: AlignmentKind::Fuzzy,
    })
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use crate::test_support::{entries, result_ids};
    use crate::{Config, Searcher};

    #[test]
    fn misspelled_prefixes_match_unfinished_long_names() {
        for name in ["algermusicplayer", "Alger Music Player"] {
            let searcher = Searcher::new(
                entries(&[name]),
                Config {
                    enable_correction: false,
                    ..Config::default()
                },
            );

            for query in [
                "agle",        // transposition with no shared adjacent character pair
                "axlg",        // insertion in a short prefix
                "axg",         // substitution with no shared adjacent character pair
                "algerxmusic", // extra character
                "algrmusic",   // missing character
                "algormusix",  // two substitutions, including the last character
                "algermusicx", // extra character at the open end
                "qlgermusic",  // typo in the first character
            ] {
                assert_eq!(
                    result_ids(&searcher, query, 5),
                    [1],
                    "query={query}, name={name}"
                );
            }

            // The same typo remains accepted throughout prefix completion.
            for end in 1..="algormusicplayer".len() {
                let query = &"algormusicplayer"[..end];

                assert_eq!(
                    result_ids(&searcher, query, 5),
                    [1],
                    "query={query}, name={name}"
                );
            }

            assert!(
                searcher
                    .search("alxormusix", 5, |_, _| Ordering::Equal)
                    .is_empty()
            );
        }
    }
}
