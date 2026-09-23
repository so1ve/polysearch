use rapidhash::{RapidHashMap, RapidHashSet};

use super::{Candidate, Index, TokenMatch, bigrams, near_pairs};
use crate::text::NormalizedText;

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

fn retain_best(ids: &mut Vec<u32>, counts: &[u32], limit: usize) {
    let compare = |a: &u32, b: &u32| {
        counts[*b as usize]
            .cmp(&counts[*a as usize])
            .then_with(|| a.cmp(b))
    };

    if ids.len() > limit {
        ids.select_nth_unstable_by(limit, compare);
        ids.truncate(limit);
    }

    ids.sort_unstable_by(compare);
}

impl Index {
    pub fn candidates(
        &self,
        query: &NormalizedText,
        limit: usize,
    ) -> impl Iterator<Item = Candidate<'_>> {
        let key = query.match_key();
        let reserved_fallbacks = usize::from(limit > 8 && query.len() >= 2) * 8;
        let primary_limit = limit - reserved_fallbacks;

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

                // The dictionary lookup already guarantees a token prefix.
                // Preserve an exact match even if another token only shares
                // its prefix; refinement needs no offsets or copied postings.
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
        for prefix_len in (2..=query.len()).rev() {
            if ids.len() == limit {
                break;
            }

            let remaining = limit - ids.len();
            let previous_len = ids.len();
            ids.extend(
                intersect(&self.token_initials, &query.chars()[..prefix_len])
                    .filter(|id| seen.insert(*id))
                    .take(remaining),
            );

            if prefix_len < query.len() && ids.len() > previous_len {
                break;
            }
        }

        // A two-character ordered pair with one skipped character is a
        // useful keyboard-like shorthand (`zd` for `zed`, `fr` for
        // `Firefox`). Keep this channel bounded and let final alignment and
        // role gates decide whether a hit is acceptable.
        if query.len() == 2 && ids.len() < limit {
            let pair = [query.chars()[0], query.chars()[1]];

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

        if query.len() < 3 && ids.len() < limit {
            let remaining = limit - ids.len();
            ids.extend(
                intersect(&self.characters, query.chars())
                    .filter(|id| seen.insert(*id))
                    .take(remaining),
            );
        }

        if query.len() >= 2 && ids.len() < limit {
            // Term IDs are dense. Query-local counters avoid hashing every
            // posting and let independent searches share the immutable index.
            let mut counts = vec![0u32; self.terms.len()];
            let mut fuzzy = Vec::new();

            for bigram in bigrams(query.chars()) {
                if let Some(matches) = self.bigrams.get(&bigram) {
                    for &id in matches {
                        if counts[id as usize] == 0 {
                            fuzzy.push(id);
                        }

                        counts[id as usize] += 1;
                    }
                }
            }

            for &id in &ids {
                counts[id as usize] = 0;
            }

            fuzzy.retain(|&id| counts[id as usize] != 0);

            retain_best(&mut fuzzy, &counts, limit - ids.len());
            ids.append(&mut fuzzy);

            if query.len() >= 3 && ids.len() < limit {
                // A typo can destroy every shared bigram in a short prefix.
                // Fill unused slots with nearby-pair matches. Every bigram
                // candidate is already selected here; exclude it from
                // recounting.
                for &id in &ids {
                    counts[id as usize] = u32::MAX;
                }

                let mut pairs = RapidHashSet::default();

                for pair in near_pairs(query.chars()) {
                    if !pairs.insert(pair) {
                        continue;
                    }

                    if let Some(matches) = self.near_pairs.get(&pair) {
                        for &id in matches {
                            if counts[id as usize] == u32::MAX {
                                continue;
                            }

                            if counts[id as usize] == 0 {
                                fuzzy.push(id);
                            }

                            counts[id as usize] += 1;
                        }
                    }
                }

                retain_best(&mut fuzzy, &counts, limit - ids.len());
                ids.extend(fuzzy);
            }
        }

        ids.into_iter().map(move |id| Candidate {
            term: &self.terms[id as usize],
            token: token_matches.remove(&id),
        })
    }
}
