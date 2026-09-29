mod candidates;
#[cfg(feature = "pinyin")]
mod pinyin;
mod terms;

use std::collections::BTreeMap;

use rapidhash::{RapidHashMap, RapidHashSet};
use smallvec::SmallVec;

use crate::{Entry, EntryId, Field, FieldId, Role};

const MAX_INDEXED_TOKENS: usize = 32;

/// A spelling and its source field, shared by retrieval and matching.
pub struct IndexedTerm {
    pub entry: EntryId,
    pub field: FieldId,
    pub role: Role,
    pub chars: Box<[char]>,
    pub token_starts: SmallVec<[u16; 4]>,
    pub penalty: u16,
    pub frequency: u32,
}

#[derive(Eq, PartialEq)]
pub enum TokenMatch {
    Prefix,
    Exact,
}

pub struct Candidate<'a> {
    pub term: &'a IndexedTerm,
    pub token: Option<TokenMatch>,
}

pub struct Index {
    terms: Vec<IndexedTerm>,
    sorted_terms: BTreeMap<String, Vec<u32>>,
    bigrams: RapidHashMap<[char; 2], Vec<u32>>,
    near_pairs: RapidHashMap<[char; 2], Vec<u32>>,
    characters: RapidHashMap<char, Vec<u32>>,
    token_initials: RapidHashMap<char, Vec<u32>>,
    tokens: BTreeMap<String, Vec<u32>>,
}

impl Index {
    pub fn new(entries: impl IntoIterator<Item = Entry>) -> Self {
        let mut index = Self {
            terms: Vec::new(),
            sorted_terms: BTreeMap::new(),
            bigrams: RapidHashMap::default(),
            near_pairs: RapidHashMap::default(),
            characters: RapidHashMap::default(),
            token_initials: RapidHashMap::default(),
            tokens: BTreeMap::new(),
        };
        let mut entry_ids = RapidHashSet::default();
        let mut field_ids = RapidHashSet::default();

        for entry in entries {
            assert!(entry_ids.insert(entry.id), "duplicate entry id");

            for field in entry.fields {
                assert!(field_ids.insert(field.id), "duplicate field id");
                index.insert(entry.id, &field);
            }
        }

        index.set_frequencies();

        index
    }

    fn insert(&mut self, entry: EntryId, field: &Field) {
        for term in terms::expand(&field.text) {
            let id = u32::try_from(self.terms.len()).expect("too many indexed terms");
            let text = term.text;
            let key = text.match_key();
            let chars = text.chars.into_boxed_slice();
            let mut token_starts = SmallVec::new();
            let mut start = 0;
            let mut retained_tokens = 0;

            self.sorted_terms.entry(key).or_default().push(id);

            for token in text.text.split_whitespace() {
                let len = token.chars().count();
                token_starts.push(start as u16);
                let initial = token.chars().next().unwrap();
                let postings = self.token_initials.entry(initial).or_default();

                if postings.last() != Some(&id) {
                    postings.push(id);
                }

                if term.penalty == 0 && len > 1 && retained_tokens < MAX_INDEXED_TOKENS {
                    self.tokens.entry(token.into()).or_default().push(id);
                    retained_tokens += 1;
                }

                start += len;
            }

            for bigram in bigrams(&chars) {
                self.bigrams.entry(bigram).or_default().push(id);
            }

            if field.role.allow_substring {
                for pair in near_pairs(&chars) {
                    let postings = self.near_pairs.entry(pair).or_default();

                    if postings.last() != Some(&id) {
                        postings.push(id);
                    }
                }
            }

            for &character in &chars {
                let postings = self.characters.entry(character).or_default();

                if postings.last() != Some(&id) {
                    postings.push(id);
                }
            }

            self.terms.push(IndexedTerm {
                entry,
                field: field.id,
                role: field.role,
                chars,
                token_starts,
                penalty: term.penalty,
                frequency: 0,
            });
        }
    }

    fn set_frequencies(&mut self) {
        // Fields from each entry are contiguous, so a last-entry ID is enough
        // to count distinct entries for each spelling and role.
        for term_ids in self.sorted_terms.values() {
            let mut frequencies = RapidHashMap::default();

            for &id in term_ids {
                let term = &self.terms[id as usize];
                let (last_entry, count) = frequencies.entry(term.role).or_insert((term.entry, 1));

                if *last_entry != term.entry {
                    *last_entry = term.entry;
                    *count += 1;
                }
            }

            for &id in term_ids {
                let term = &mut self.terms[id as usize];
                term.frequency = frequencies[&term.role].1;
            }
        }
    }

    pub fn vocabulary(&self) -> Vec<&str> {
        let mut words: Vec<_> = self
            .sorted_terms
            .keys()
            .chain(self.tokens.keys())
            .map(String::as_str)
            .collect();
        words.sort_unstable();
        words.dedup();

        words
    }
}

fn bigrams(chars: &[char]) -> impl Iterator<Item = [char; 2]> {
    chars.windows(2).map(|pair| [pair[0], pair[1]])
}

fn near_pairs(chars: &[char]) -> impl Iterator<Item = [char; 2]> {
    chars.iter().enumerate().flat_map(move |(first, &left)| {
        chars[first + 1..]
            .iter()
            .take(2)
            .map(move |&right| [left, right])
    })
}

#[cfg(test)]
mod tests {
    use crate::test_support::{entry, result_ids};
    use crate::{ALIAS, Config, PRIMARY_NAME, Searcher};

    #[test]
    fn frequency_counts_distinct_entries_separately_for_each_role() {
        for duplicate_entry in [false, true] {
            let mut entries = vec![
                entry(
                    1,
                    vec![(1, PRIMARY_NAME, "Firefox"), (2, PRIMARY_NAME, "Firefox")],
                ),
                entry(2, vec![(3, PRIMARY_NAME, "Firefax")]),
                entry(3, vec![(4, ALIAS, "Firefox")]),
            ];

            if duplicate_entry {
                entries.push(entry(4, vec![(5, PRIMARY_NAME, "Firefox")]));
            }

            let config = Config {
                enable_correction: false,
                ..Config::default()
            };
            let searcher = Searcher::new(entries, config);

            assert_eq!(
                result_ids(&searcher, "firef", 5),
                if duplicate_entry {
                    vec![2, 1, 4, 3]
                } else {
                    vec![1, 2, 3]
                },
            );
        }
    }
}
