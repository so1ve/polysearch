#[cfg(feature = "pinyin")]
mod pinyin;
mod terms;

use rapidhash::{RapidHashMap, RapidHashSet};
use smallvec::SmallVec;

use crate::{Entry, EntryId, FieldId, Role};

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Kind {
    Literal,
    #[cfg(feature = "pinyin")]
    Pinyin,
    #[cfg(feature = "pinyin")]
    Initials,
}

pub struct Source {
    pub entry: usize,
    pub field: FieldId,
    pub role: Role,
    pub kind: Kind,
}

pub struct Spelling {
    pub text: Box<str>,
    pub sources: SmallVec<[Source; 1]>,
}

impl AsRef<str> for Spelling {
    fn as_ref(&self) -> &str {
        &self.text
    }
}

pub struct Index {
    pub entries: Vec<EntryId>,
    pub spellings: Vec<Spelling>,
}

impl Index {
    pub fn new(entries: impl IntoIterator<Item = Entry>) -> Self {
        let mut ids = Vec::new();
        let mut entry_ids = RapidHashSet::default();
        let mut field_ids = RapidHashSet::default();
        let mut spellings: RapidHashMap<String, SmallVec<[Source; 1]>> = RapidHashMap::default();

        for entry in entries {
            assert!(entry_ids.insert(entry.id), "duplicate entry id");

            let index = ids.len();
            ids.push(entry.id);

            for field in entry.fields {
                assert!(field_ids.insert(field.id), "duplicate field id");

                terms::expand(&field.text, |text, kind| {
                    let sources = spellings.entry(text).or_default();

                    // Field variants arrive together, strongest kind first.
                    if sources.last().is_none_or(|source| source.field != field.id) {
                        sources.push(Source {
                            entry: index,
                            field: field.id,
                            role: field.role,
                            kind,
                        });
                    }
                });
            }
        }

        let spellings = spellings
            .into_iter()
            .map(|(text, sources)| Spelling {
                text: text.into_boxed_str(),
                sources,
            })
            .collect();

        Self {
            entries: ids,
            spellings,
        }
    }
}
