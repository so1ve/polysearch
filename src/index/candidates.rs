use rapidhash::{RapidHashMap, RapidHashSet};

use super::{Candidate, Index, TokenMatch, bigrams, near_pairs};
use crate::text::NormalizedText;

const FUZZY_RESERVE: usize = 8;
const SELECTED: u32 = u32::MAX;

impl Index {
    pub fn candidates(
        &self,
        query: &NormalizedText,
        limit: usize,
    ) -> impl Iterator<Item = Candidate<'_>> {
        let key = query.match_key();

        // A finite cap leaves room for typo and abbreviation candidates.
        let primary_limit = if limit > FUZZY_RESERVE && query.chars.len() >= 2 {
            limit - FUZZY_RESERVE
        } else {
            limit
        };

        // Each term belongs to exactly one key, so prefix postings are unique.
        let mut ids: Vec<_> = self
            .sorted_terms
            .range(key.clone()..)
            .take_while(|(text, _)| text.starts_with(&key))
            .flat_map(|(_, term_ids)| term_ids.iter().copied())
            .take(primary_limit)
            .collect();

        let mut token_matches = RapidHashMap::default();
        let mut seen: RapidHashSet<_> = ids.iter().copied().collect();

        for (token, postings) in self
            .tokens
            .range(key.clone()..)
            .take_while(|(text, _)| text.starts_with(&key))
        {
            for &id in postings {
                if !seen.contains(&id) {
                    if ids.len() == primary_limit {
                        break;
                    }

                    seen.insert(id);
                    ids.push(id);
                }

                // An exact token match wins over other tokens sharing its
                // prefix.
                let matched = token_matches.entry(id).or_insert(TokenMatch::Prefix);
                if token == &key {
                    *matched = TokenMatch::Exact;
                }
            }

            if ids.len() == primary_limit {
                break;
            }
        }

        // Recall full initials, then the longest initials prefix with new
        // candidates (`vscd` can continue inside Code). Alignment checks order
        // and token boundaries.
        for prefix_len in (2..=query.chars.len()).rev() {
            if ids.len() == limit {
                break;
            }

            let remaining = limit - ids.len();
            let previous_len = ids.len();
            ids.extend(
                intersect(&self.token_initials, &query.chars[..prefix_len])
                    .filter(|id| seen.insert(*id))
                    .take(remaining),
            );

            if prefix_len < query.chars.len() && ids.len() > previous_len {
                break;
            }
        }

        // Recall short shorthands such as `zd` for `zed`; matching checks
        // their position and field role.
        if query.chars.len() == 2 && ids.len() < limit {
            let pair = [query.chars[0], query.chars[1]];

            if let Some(postings) = self.near_pairs.get(&pair) {
                let remaining = limit - ids.len();
                ids.extend(
                    postings
                        .iter()
                        .copied()
                        .filter(|id| seen.insert(*id))
                        .take(remaining),
                );
            }
        }

        if query.chars.len() < 3 && ids.len() < limit {
            let remaining = limit - ids.len();
            ids.extend(
                intersect(&self.characters, &query.chars)
                    .filter(|id| seen.insert(*id))
                    .take(remaining),
            );
        }

        if query.chars.len() >= 2 && ids.len() < limit {
            let mut counts = vec![0; self.terms.len()];
            let mut pending = Vec::new();

            for &id in &ids {
                counts[id as usize] = SELECTED;
            }

            extend_shared(
                bigrams(&query.chars).filter_map(|pair| self.bigrams.get(&pair)),
                &mut counts,
                &mut pending,
                &mut ids,
                limit,
            );

            // A typo can destroy every shared bigram in a short prefix.
            // Nearby pairs fill the remaining slots, without counting a
            // repeated query pair or an already selected term twice.
            if query.chars.len() >= 3 && ids.len() < limit {
                let mut pairs = RapidHashSet::default();
                extend_shared(
                    near_pairs(&query.chars)
                        .filter(|pair| pairs.insert(*pair))
                        .filter_map(|pair| self.near_pairs.get(&pair)),
                    &mut counts,
                    &mut pending,
                    &mut ids,
                    limit,
                );
            }
        }

        ids.into_iter().map(move |id| Candidate {
            term: &self.terms[id as usize],
            token: token_matches.remove(&id),
        })
    }
}

fn intersect<'a>(
    index: &'a RapidHashMap<char, Vec<u32>>,
    query: &[char],
) -> impl Iterator<Item = u32> + 'a {
    let mut postings = query
        .iter()
        .map(|character| index.get(character))
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default();
    postings.sort_unstable_by_key(|posting| posting.len());
    let shortest = postings
        .first()
        .map(|posting| posting.as_slice())
        .unwrap_or(&[]);

    // Posting IDs follow index insertion order and are therefore sorted.
    shortest.iter().copied().filter(move |id| {
        postings
            .iter()
            .skip(1)
            .all(|posting| posting.binary_search(id).is_ok())
    })
}

// Reuse the counters and pending buffer across both pair-retrieval passes.
fn extend_shared<'a>(
    postings: impl Iterator<Item = &'a Vec<u32>>,
    counts: &mut [u32],
    pending: &mut Vec<u32>,
    selected: &mut Vec<u32>,
    limit: usize,
) {
    for posting in postings {
        for &id in posting {
            let count = &mut counts[id as usize];

            if *count == SELECTED {
                continue;
            }

            if *count == 0 {
                pending.push(id);
            }

            *count += 1;
        }
    }

    let compare = |left: &u32, right: &u32| {
        counts[*right as usize]
            .cmp(&counts[*left as usize])
            .then_with(|| left.cmp(right))
    };
    let remaining = limit - selected.len();

    if pending.len() > remaining {
        pending.select_nth_unstable_by(remaining, compare);
        pending.truncate(remaining);
    }

    pending.sort_unstable_by(compare);

    // Another pass runs only if every pending candidate fits. All nonzero
    // counters are then marked selected, so none need clearing or recounting.
    for &id in pending.iter() {
        counts[id as usize] = SELECTED;
    }

    selected.append(pending);
}
