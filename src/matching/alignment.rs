use std::cmp::Reverse;

use super::subsequence;
use crate::distance::{MatchMode, edit_distance};

pub enum AlignmentKind {
    Contiguous,
    Subsequence,
    Abbreviation { initials_only: bool },
    Fuzzy,
}

pub struct Alignment {
    pub edits: u16,
    pub gaps: u16,
    pub unmatched: u16,
    pub start: u16,
    pub kind: AlignmentKind,
}

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
            && matches!(subsequence.kind, AlignmentKind::Abbreviation { .. })
        {
            return Some(subsequence);
        }

        return Some(contiguous);
    }

    subsequence::align(query, candidate, token_starts)
        .or_else(|| fuzzy(query, candidate, token_starts, max_edits))
}

fn fuzzy(
    query: &[char],
    candidate: &[char],
    token_starts: &[u16],
    max_edits: u16,
) -> Option<Alignment> {
    let max_edits = max_edits.min((query.len() / 3) as u16);

    if max_edits == 0 {
        return None;
    }

    // Prefer word prefixes, including ones that continue into later words.
    let mut best = token_starts
        .iter()
        .filter_map(|&start| {
            let start = usize::from(start);
            let (edits, span) =
                edit_distance(query, &candidate[start..], max_edits, MatchMode::Prefix)?;

            Some((edits, Reverse(span.len()), start))
        })
        .min();

    // Exact matches were handled above. An internal fragment can only beat
    // a word prefix by using fewer edits, so a one-edit prefix is enough.
    let max_edits = best.map(|(edits, ..)| edits - 1).unwrap_or(max_edits);

    if max_edits > 0 {
        for (token, &start) in token_starts.iter().enumerate() {
            let start = usize::from(start);
            let end = token_starts
                .get(token + 1)
                .map(|&end| usize::from(end))
                .unwrap_or(candidate.len());

            // Internal fragments stay within a word; they must not stitch
            // unrelated words together, such as `amd` in `Program Loader`.
            if let Some((edits, span)) = edit_distance(
                query,
                &candidate[start..end],
                max_edits,
                MatchMode::Substring,
            ) {
                let matched = (edits, Reverse(span.len()), start + span.start);

                if best.is_none_or(|previous| matched < previous) {
                    best = Some(matched);
                }
            }
        }
    }

    best.map(|(edits, Reverse(consumed), start)| Alignment {
        edits,
        gaps: 0,
        unmatched: (candidate.len() - consumed) as u16,
        start: start as u16,
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
