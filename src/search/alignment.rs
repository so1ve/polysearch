mod subsequence;

use crate::distance::{MatchLength, damerau_levenshtein};

enum AlignmentKind {
    Contiguous {
        crosses_token_boundary: bool,
        token_starts: bool,
    },
    Subsequence {
        crosses_token_boundary: bool,
        token_starts: bool,
        token_progression: bool,
    },
    Fuzzy,
}

pub struct Alignment {
    pub edits: u16,
    pub gaps: u16,
    pub start: u16,
    kind: AlignmentKind,
}

impl Alignment {
    pub const fn is_contiguous(&self) -> bool {
        matches!(self.kind, AlignmentKind::Contiguous { .. })
    }

    pub const fn crosses_token_boundary(&self) -> bool {
        match self.kind {
            AlignmentKind::Contiguous {
                crosses_token_boundary,
                ..
            }
            | AlignmentKind::Subsequence {
                crosses_token_boundary,
                ..
            } => crosses_token_boundary,
            AlignmentKind::Fuzzy => false,
        }
    }

    pub const fn token_starts(&self) -> bool {
        match self.kind {
            AlignmentKind::Contiguous { token_starts, .. }
            | AlignmentKind::Subsequence { token_starts, .. } => token_starts,
            AlignmentKind::Fuzzy => false,
        }
    }

    pub const fn token_progression(&self) -> bool {
        matches!(
            self.kind,
            AlignmentKind::Subsequence {
                token_progression: true,
                ..
            }
        ) || self.is_contiguous()
    }
}

fn capped(value: usize) -> u16 {
    value.min(u16::MAX as usize) as u16
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
            gaps: capped(candidate.len() - query_len),
            start: capped(start),
            kind: AlignmentKind::Contiguous {
                crosses_token_boundary: token_starts.iter().any(|&boundary| {
                    usize::from(boundary) > start && usize::from(boundary) < start + query_len
                }),
                token_starts: (start..start + query_len)
                    .all(|position| token_starts.contains(&capped(position))),
            },
        };

        // A later token-initial path can be more meaningful than an earlier
        // incidental substring (`ce` in `Office Code Editor`). Only inspect
        // that alternate path when the term actually has multiple tokens;
        // single-token substrings keep their cheap direct alignment.
        if start == 0 || token_starts.len() <= 1 {
            return Some(contiguous);
        }

        if let Some(subsequence) = subsequence::align(query, candidate, token_starts)
            && subsequence.crosses_token_boundary()
            && subsequence.token_progression()
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
        gaps: capped(candidate.len() - end),
        start: 0,
        kind: AlignmentKind::Fuzzy,
    })
}

#[cfg(test)]
mod tests {
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

            assert!(searcher.search("alxormusix", 5).is_empty());
        }
    }
}
