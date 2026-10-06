//! An in-memory fuzzy search library for launchers, command palettes, and local
//! search.
//!
//! Uses Frizbee for fuzzy matching, with field weighting and optional pinyin.
//!
//! # Usage
//!
//! Build a [`Searcher`] once and reuse it for queries:
//!
//! ```
//! use polysearch::{Config, Entry, Field, PRIMARY_NAME, Searcher};
//!
//! let searcher = Searcher::new(
//!     [Entry {
//!         id: 1,
//!         fields: vec![Field {
//!             id: 1,
//!             role: PRIMARY_NAME,
//!             text: "文档助手".into(),
//!         }],
//!     }],
//!     Config::default(),
//! );
//!
//! let results = searcher.search("文档", 10, |_| 0);
//! assert_eq!(results[0].entry, 1);
//! assert_eq!(results[0].field, 1);
//! ```
//!
//! # Entries and results
//!
//! An [`Entry`] contains one or more [`Field`] values. Choose a [`Role`] from
//! [`PRIMARY_NAME`], [`LOCALIZED_NAME`], [`ALIAS`], [`KEYWORD`], and
//! [`IDENTIFIER`], in descending preference.
//!
//! [`EntryId`] values must be unique across the index, as must [`FieldId`]
//! values. Duplicate IDs cause [`Searcher::new`] to panic.
//!
//! [`Searcher::search`] returns results ranked by relevance. Each matching
//! entry appears once, alongside its best matching field ID. The second
//! argument limits the result count; an empty query returns no results. Rebuild
//! the searcher when the indexed data changes.
//!
//! Exact names and aliases rank first. Other results use Frizbee match quality
//! and how much of a name or word the query covers, with small adjustments for
//! field role and spelling confidence. A clear keyword match can outrank a
//! weak name match. Full pinyin and initials support the same fuzzy matching
//! as literal text.
//!
//! The callback supplies a preference from 0 to 255, such as usage history.
//! Its bonus is capped at 5% of a perfect completion's score. It runs once per
//! matching entry; return zero to use built-in ordering alone.
//!
//! Spaces separate query words, all of which must match the same field variant,
//! in any order. Names also keep a compact spelling for input without spaces.
//!
//! # Configuration
//!
//! [`Config::max_typos`] defaults to `1`: Frizbee may leave one query character
//! unmatched per query word. Set it to `0` to require an ordered subsequence.
//! This is not an edit-distance limit: skipped characters in a candidate do not
//! count as typos. Matching is case-insensitive. All Frizbee matches
//! participate in ranking; Polysearch does not apply a minimum score cutoff.
//!
//! # Features
//!
//! - **`pinyin`**: matches Chinese names by full pinyin and initials.

mod index;
mod matching;
mod search;
#[cfg(test)]
mod test_support;
mod text;

pub use search::{SearchResult, Searcher};

pub type EntryId = u64;
pub type FieldId = u32;

/// A field's purpose and ranking preference.
///
/// Use one of the built-in constants; roles cannot be customized.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Role(u8);

/// The entry's main display name.
pub const PRIMARY_NAME: Role = Role(0);

/// An alternate localized name.
pub const LOCALIZED_NAME: Role = Role(1);

/// An alternate name or synonym.
pub const ALIAS: Role = Role(2);

/// Keywords or descriptive text.
pub const KEYWORD: Role = Role(3);

/// A technical identifier, such as a desktop file ID.
pub const IDENTIFIER: Role = Role(5);

#[derive(Clone, Debug)]
pub struct Field {
    pub id: FieldId,
    pub role: Role,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub id: EntryId,
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug)]
pub struct Config {
    /// Maximum unmatched characters per query word. Defaults to one.
    /// Zero requires an ordered subsequence, allowing gaps in the candidate.
    pub max_typos: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self { max_typos: 1 }
    }
}
