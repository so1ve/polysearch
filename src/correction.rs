use arrayvec::{ArrayString, ArrayVec};
use rapidhash::RapidHashSet;
use rapidhash::fast::RandomState as RapidState;
use smallvec::SmallVec;
use string_interner::backend::StringBackend;
use string_interner::symbol::SymbolU32;
use string_interner::{StringInterner, Symbol};

use crate::distance::{MatchMode, edit_distance};

const MAX_INDEXED_EDITS: u16 = 2;
const DELETE_PREFIX_LENGTH: usize = 7;
const MAX_DELETE_KEY_BYTES: usize = DELETE_PREFIX_LENGTH * 4;
const MAX_DELETE_KEYS: usize =
    1 + DELETE_PREFIX_LENGTH + DELETE_PREFIX_LENGTH * (DELETE_PREFIX_LENGTH - 1) / 2;

type DeleteKey = ArrayString<MAX_DELETE_KEY_BYTES>;
type PostingList = SmallVec<[SymbolU32; 1]>;
type Interner = StringInterner<StringBackend<SymbolU32>, RapidState>;

/// Looks up bounded edit-distance variants in the finished index vocabulary.
pub struct CorrectionIndex {
    max_edits: u16,
    terms: Interner,
    delete_keys: Interner,
    postings: Vec<PostingList>,
}

impl CorrectionIndex {
    pub fn new(vocabulary: &[&str], max_edits: u16) -> Self {
        let mut corrector = Self {
            max_edits,
            terms: Interner::with_hasher(RapidState::default()),
            delete_keys: Interner::with_hasher(RapidState::default()),
            postings: Vec::new(),
        };

        for term in vocabulary {
            let term_symbol = corrector.terms.get_or_intern(term);

            // Larger budgets use the complete vocabulary instead of an
            // exponentially larger deletion index.
            if max_edits > MAX_INDEXED_EDITS {
                continue;
            }

            let chars: Vec<_> = term.chars().collect();

            for key in delete_keys(&chars, max_edits) {
                let delete_symbol = corrector.delete_keys.get_or_intern(key.as_str());
                let posting = delete_symbol.to_usize();

                if posting == corrector.postings.len() {
                    corrector.postings.push(PostingList::new());
                }

                corrector.postings[posting].push(term_symbol);
            }
        }

        corrector
    }

    pub fn correct(&self, query: &[char]) -> Option<(&str, u16)> {
        let mut best = None;
        let mut best_cost = u16::MAX;
        let mut consider = |symbol, term: &str| {
            let chars: Vec<_> = term.chars().collect();
            let Some((cost, _)) = edit_distance(query, &chars, self.max_edits, MatchMode::Full)
            else {
                return;
            };

            if cost == 0 || cost > best_cost {
                return;
            }

            // An equally close word makes the correction ambiguous. Only a
            // strictly better word can make it unambiguous again.
            best = (cost < best_cost).then_some(symbol);
            best_cost = cost;
        };

        if self.max_edits > MAX_INDEXED_EDITS {
            for (symbol, term) in self.terms.iter() {
                consider(symbol, term);
            }
        } else {
            let mut seen = RapidHashSet::default();

            for key in delete_keys(query, self.max_edits) {
                let Some(delete_symbol) = self.delete_keys.get(key.as_str()) else {
                    continue;
                };

                for &term_symbol in &self.postings[delete_symbol.to_usize()] {
                    if !seen.insert(term_symbol) {
                        continue;
                    }

                    let term = self.terms.resolve(term_symbol).unwrap();
                    consider(term_symbol, term);
                }
            }
        }

        best.map(|symbol| (self.terms.resolve(symbol).unwrap(), best_cost))
    }
}

fn delete_keys(chars: &[char], max_edits: u16) -> ArrayVec<DeleteKey, MAX_DELETE_KEYS> {
    let prefix = &chars[..chars.len().min(DELETE_PREFIX_LENGTH)];
    let mut keys = ArrayVec::new();
    let mut add = |first: Option<usize>, second: Option<usize>| {
        let mut key = DeleteKey::new();

        for (index, &character) in prefix.iter().enumerate() {
            if Some(index) != first && Some(index) != second {
                key.push(character);
            }
        }

        if !keys.contains(&key) {
            keys.push(key);
        }
    };

    add(None, None);

    for first in 0..prefix.len() {
        add(Some(first), None);

        if max_edits > 1 {
            for second in first + 1..prefix.len() {
                add(Some(first), Some(second));
            }
        }
    }

    keys
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use crate::test_support::{entry, result_ids};
    use crate::{Config, IDENTIFIER, Searcher};

    #[test]
    fn correction_does_not_turn_a_token_into_a_prefix_query() {
        for names in [
            ["amdb", "amdbuild", "amdbase"],
            ["org.amdb.amdbuild", "org.amdbuild", "org.amdbase"],
        ] {
            let entries = names
                .into_iter()
                .zip(1..)
                .map(|(name, id)| entry(u64::from(id), vec![(id, IDENTIFIER, name)]));
            let searcher = Searcher::new(entries, Config::default());

            assert_eq!(result_ids(&searcher, "amdm", 5), [1], "names={names:?}");
        }
    }

    #[test]
    fn ambiguous_corrections_do_not_expand_results() {
        let searcher = Searcher::new(
            [
                entry(1, vec![(1, IDENTIFIER, "small")]),
                entry(2, vec![(2, IDENTIFIER, "smart")]),
                entry(3, vec![(3, IDENTIFIER, "sxs")]),
                entry(4, vec![(4, IDENTIFIER, "sns")]),
            ],
            Config::default(),
        );

        assert_eq!(result_ids(&searcher, "sma", 10), [1, 2]);
        assert!(
            searcher
                .search("sms", 10, |_, _| Ordering::Equal)
                .is_empty()
        );
    }

    #[test]
    fn correction_budget_can_exceed_the_deletion_index_depth() {
        for (max_edits, expected) in [(2, vec![]), (3, vec![1])] {
            let searcher = Searcher::new(
                [entry(1, vec![(1, IDENTIFIER, "Visual Studio")])],
                Config {
                    max_edits,
                    ..Config::default()
                },
            );

            assert_eq!(result_ids(&searcher, "bisyalatudio", 5), expected);
        }
    }
}
