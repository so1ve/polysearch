use super::{Alignment, AlignmentKind, capped};

#[derive(Clone, Copy)]
struct SubsequencePath {
    first: usize,
    last: usize,
    token_starts: usize,
    adjacent_steps: usize,
}

impl SubsequencePath {
    fn extend(self, position: usize, token_start: bool) -> Self {
        Self {
            first: self.first,
            last: position,
            token_starts: self.token_starts + usize::from(token_start),
            adjacent_steps: self.adjacent_steps + usize::from(position == self.last + 1),
        }
    }

    fn retain(self, best: &mut Option<Self>) {
        // All extensions of these paths end at the same position. A later
        // first character then gives a shorter span, independent of the
        // previous endpoint. Adjacent extensions are considered separately.
        let key = |path: Self| (path.token_starts, path.adjacent_steps, path.first);

        if best.is_none_or(|previous| key(self) > key(previous)) {
            *best = Some(self);
        }
    }
}

pub fn align(query: &[char], candidate: &[char], token_starts: &[u16]) -> Option<Alignment> {
    if token_starts.len() <= 1 {
        return greedy(query, candidate, token_starts);
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
    let mut paths = vec![[None; 2]; candidate.len()];
    let mut next = paths.clone();

    for (position, &character) in candidate.iter().enumerate() {
        if character == query[0] {
            let path = SubsequencePath {
                first: position,
                last: position,
                token_starts: usize::from(starts[position]),
                adjacent_steps: 0,
            };
            paths[position][0] = Some(path);

            if starts[position] {
                paths[position][1] = Some(path);
            }
        }
    }

    // Prefix-best states make every row linear in candidate length. There is
    // no query-length threshold that changes the matching semantics.
    for &character in &query[1..] {
        let mut local_best: Option<SubsequencePath> = None;
        let mut anchored_best: Option<SubsequencePath> = None;
        let mut anchored_current: Option<SubsequencePath> = None;

        for (position, &candidate_character) in candidate.iter().enumerate() {
            let token_start = starts[position];

            if token_start {
                if let Some(path) = anchored_current.take() {
                    path.retain(&mut anchored_best);
                }

                local_best = None;
            } else if position > 0 {
                for path in paths[position - 1].iter().flatten() {
                    path.retain(&mut local_best);
                }

                if let Some(path) = paths[position - 1][1] {
                    path.retain(&mut anchored_current);
                }
            }

            next[position] = [None; 2];

            if candidate_character != character {
                continue;
            }

            if let Some(path) = local_best {
                path.extend(position, token_start)
                    .retain(&mut next[position][0]);
            }

            if token_start {
                if let Some(path) = anchored_best {
                    path.extend(position, true).retain(&mut next[position][1]);
                }
            } else if let Some(path) = anchored_current {
                path.extend(position, false).retain(&mut next[position][1]);
            }
        }

        std::mem::swap(&mut paths, &mut next);
    }

    let mut best = None;

    for [local, anchored] in paths {
        for (kind, path) in [local, anchored].into_iter().enumerate() {
            let Some(path) = path else {
                continue;
            };

            let key = (
                kind,
                path.token_starts,
                path.adjacent_steps,
                std::cmp::Reverse(path.last - path.first),
                std::cmp::Reverse(path.first),
            );

            if best
                .as_ref()
                .is_none_or(|(_, previous_key)| key > *previous_key)
            {
                best = Some((path, key));
            }
        }
    }

    let (path, key) = best?;
    let anchored = key.0 == 1;

    let crosses_token_boundary = token_starts
        .iter()
        .any(|&boundary| usize::from(boundary) > path.first && usize::from(boundary) <= path.last);

    Some(Alignment {
        edits: 0,
        gaps: capped(path.last - path.first + 1 - query.len()),
        start: capped(path.first),
        kind: AlignmentKind::Subsequence {
            crosses_token_boundary,
            token_starts: path.token_starts == query.len(),
            token_progression: anchored,
        },
    })
}

fn greedy(query: &[char], candidate: &[char], token_starts: &[u16]) -> Option<Alignment> {
    let mut candidate_index = 0;
    let mut first = None;
    let mut last = 0;
    let mut token_starts_count = 0;

    for character in query {
        let offset = candidate[candidate_index..]
            .iter()
            .position(|item| item == character)?;
        let found = candidate_index + offset;
        first.get_or_insert(found);
        last = found;
        token_starts_count += usize::from(token_starts.contains(&capped(found)));
        candidate_index = found + 1;
    }

    let first = first?;

    Some(Alignment {
        edits: 0,
        gaps: capped(last - first + 1 - query.len()),
        start: capped(first),
        kind: AlignmentKind::Subsequence {
            crosses_token_boundary: false,
            token_starts: token_starts_count == query.len(),
            token_progression: false,
        },
    })
}

#[cfg(test)]
mod tests {
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
        assert!(searcher.search("amd", 5).is_empty());
    }
}
