use super::alignment::{Alignment, AlignmentKind};

#[derive(Clone, Copy)]
struct Path {
    first: usize,
    last: usize,
    initials: usize,
    adjacent: usize,
}

impl Path {
    fn extend(self, position: usize, token_start: bool) -> Self {
        Self {
            first: self.first,
            last: position,
            initials: self.initials + usize::from(token_start),
            adjacent: self.adjacent + usize::from(position == self.last + 1),
        }
    }

    fn retain(self, best: &mut Option<Self>) {
        // Prefer more token initials and adjacent steps. For otherwise
        // equal paths, a later first character gives the shorter span.
        let key = |path: Self| (path.initials, path.adjacent, path.first);

        if best.is_none_or(|previous| key(self) > key(previous)) {
            *best = Some(self);
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Paths {
    local: Option<Path>,
    anchored: Option<Path>,
}

pub fn align(query: &[char], candidate: &[char], token_starts: &[u16]) -> Option<Alignment> {
    if token_starts.len() <= 1 {
        return greedy(query, candidate);
    }

    if !candidate.contains(&query[0]) {
        return None;
    }

    let mut starts = vec![false; candidate.len()];

    for &start in token_starts {
        starts[usize::from(start)] = true;
    }

    // One path stays inside a token; the other starts at a token boundary and
    // may enter subsequent tokens only at their first character. Keeping both
    // prevents an earlier internal letter from hiding a valid acronym, as the
    // first 's' in Visual would do for "vsc".
    let mut paths = vec![Paths::default(); candidate.len()];
    let mut next = paths.clone();

    for (position, &character) in candidate.iter().enumerate() {
        if character == query[0] {
            let path = Path {
                first: position,
                last: position,
                initials: usize::from(starts[position]),
                adjacent: 0,
            };
            paths[position].local = Some(path);

            if starts[position] {
                paths[position].anchored = Some(path);
            }
        }
    }

    // Prefix-best states make every row linear in candidate length. There is
    // no query-length threshold that changes the matching semantics.
    for &character in &query[1..] {
        let mut local_best: Option<Path> = None;
        let mut earlier_tokens: Option<Path> = None;
        let mut current_token: Option<Path> = None;

        for (position, &candidate_character) in candidate.iter().enumerate() {
            let token_start = starts[position];

            if token_start {
                if let Some(path) = current_token.take() {
                    path.retain(&mut earlier_tokens);
                }

                local_best = None;
            } else if position > 0 {
                if let Some(path) = paths[position - 1].local {
                    path.retain(&mut local_best);
                }

                if let Some(path) = paths[position - 1].anchored {
                    path.retain(&mut current_token);
                }
            }

            next[position] = Paths::default();

            if candidate_character != character {
                continue;
            }

            if let Some(path) = local_best {
                path.extend(position, token_start)
                    .retain(&mut next[position].local);
            }

            if token_start {
                if let Some(path) = earlier_tokens {
                    path.extend(position, true)
                        .retain(&mut next[position].anchored);
                }
            } else if let Some(path) = current_token {
                path.extend(position, false)
                    .retain(&mut next[position].anchored);
            }
        }

        std::mem::swap(&mut paths, &mut next);
    }

    let (path, anchored) = paths
        .iter()
        .flat_map(|paths| {
            [
                (paths.local.as_ref(), false),
                (paths.anchored.as_ref(), true),
            ]
        })
        .filter_map(|(path, anchored)| path.map(|path| (path, anchored)))
        .max_by_key(|(path, anchored)| {
            (
                *anchored,
                path.initials,
                path.adjacent,
                std::cmp::Reverse(path.last - path.first),
                std::cmp::Reverse(path.first),
            )
        })?;

    // An anchored path can enter another token only through its initial.
    let kind = if anchored && path.initials > 1 {
        AlignmentKind::Abbreviation {
            initials_only: path.initials == query.len(),
        }
    } else {
        AlignmentKind::Subsequence
    };

    Some(Alignment {
        edits: 0,
        gaps: (path.last - path.first + 1 - query.len()) as u16,
        unmatched: (candidate.len() - (path.last - path.first + 1)) as u16,
        start: path.first as u16,
        kind,
    })
}

fn greedy(query: &[char], candidate: &[char]) -> Option<Alignment> {
    let first = candidate
        .iter()
        .position(|&character| character == query[0])?;
    let mut last = first;

    for character in &query[1..] {
        let offset = candidate[last + 1..]
            .iter()
            .position(|item| item == character)?;
        last += offset + 1;
    }

    Some(Alignment {
        edits: 0,
        gaps: (last - first + 1 - query.len()) as u16,
        unmatched: (candidate.len() - (last - first + 1)) as u16,
        start: first as u16,
        kind: AlignmentKind::Subsequence,
    })
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use crate::test_support::{entries, result_ids};
    use crate::{Config, Searcher};

    #[test]
    fn cross_word_subsequences_must_follow_token_initials() {
        let searcher = Searcher::new(
            entries(&["Visual Studio Code", "Wine Windows Program Loader"]),
            Config {
                enable_correction: false,
                max_edits: 0,
                ..Config::default()
            },
        );

        assert_eq!(result_ids(&searcher, "vsc", 5), [1]);
        assert_eq!(result_ids(&searcher, "vscd", 5), [1]);
        assert!(searcher.search("amd", 5, |_, _| Ordering::Equal).is_empty());
    }
}
