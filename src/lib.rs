//! An in-memory fuzzy search library for launchers, command palettes, and local
//! search.
//!
//! Supports prefixes, substrings, abbreviations, and common typos.
//!
//! # Usage
//!
//! Build a [`Searcher`] once and reuse it for queries:
//!
//! ```
//! use std::cmp::Ordering;
//!
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
//! let results = searcher.search("文档", 10, |_, _| Ordering::Equal);
//! assert_eq!(results[0].entry, 1);
//! assert_eq!(results[0].field, 1);
//! ```
//!
//! # Entries and results
//!
//! An [`Entry`] contains one or more [`Field`] values. Choose a [`Role`] from
//! [`PRIMARY_NAME`], [`LOCALIZED_NAME`], [`ALIAS`], [`KEYWORD`], and
//! [`IDENTIFIER`]. Matching rules and ranking priorities are fixed by the
//! library; custom roles are not supported.
//!
//! [`EntryId`] values must be unique across the index, as must [`FieldId`]
//! values. Duplicate IDs cause [`Searcher::new`] to panic.
//!
//! [`Searcher::search`] returns results ranked by relevance. Each matching
//! entry appears once, alongside its best matching field ID. The second
//! argument limits the result count; an empty query returns no results. Rebuild
//! the searcher when the indexed data changes.
//!
//! Results are grouped into three tiers:
//!
//! 1. Exact names or whole tokens, without spelling edits.
//! 2. Prefixes, contiguous substrings, and token-initial abbreviations.
//! 3. Typo corrections and loose subsequences.
//!
//! Pinyin follows the same rules. Within each tier, the comparison callback
//! applies usage history or other preferences before built-in match details.
//! Return [`std::cmp::Ordering::Equal`] to keep the built-in ordering.
//!
//! # Configuration
//!
//! Start with [`Config::default()`]:
//!
//! - [`Config::max_edits`] defaults to `2`. Set it to `0` to disable edit
//!   tolerance while keeping prefix, abbreviation, and subsequence matching.
//! - [`Config::enable_correction`] defaults to `true`. Disabling it skips
//!   dictionary correction; direct fuzzy matching still follows `max_edits`.
//! - [`Config::max_candidates`] is unlimited by default. Setting a cap reduces
//!   work but may miss matches.
//!
//! # Features
//!
//! - **`pinyin`**: matches Chinese names by full pinyin and initials.

mod correction;
mod distance;
mod index;
mod matching;
mod search;
#[cfg(test)]
mod test_support;
mod text;

pub use search::{SearchResult, Searcher};

pub type EntryId = u64;
pub type FieldId = u32;

/// A field's purpose, with fixed matching rules and ranking priority.
///
/// Use one of the built-in constants; roles cannot be customized.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Role {
    priority: u8,
    min_query_chars: u8,
    allow_substring: bool,
    allow_correction: bool,
}

/// The entry's main display name.
pub const PRIMARY_NAME: Role = Role {
    priority: 0,
    min_query_chars: 1,
    allow_substring: true,
    allow_correction: true,
};

/// An alternate localized name.
pub const LOCALIZED_NAME: Role = Role {
    priority: 1,
    ..PRIMARY_NAME
};

/// An alternate name or synonym.
pub const ALIAS: Role = Role {
    priority: 2,
    min_query_chars: 2,
    ..PRIMARY_NAME
};

/// Keywords or descriptive text; excluded from dictionary correction.
pub const KEYWORD: Role = Role {
    priority: 3,
    min_query_chars: 2,
    allow_substring: false,
    allow_correction: false,
};

/// A technical identifier, matched by prefixes and whole-token correction.
pub const IDENTIFIER: Role = Role {
    priority: 5,
    min_query_chars: 3,
    allow_substring: false,
    allow_correction: true,
};

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
    /// Maximum terms evaluated per retrieval pass, including a correction
    /// pass when needed. Defaults to no truncation; a finite cap trades
    /// recall for less work. Results are limited by `Searcher::search`.
    pub max_candidates: usize,
    /// Maximum total edits from query correction and fuzzy alignment.
    /// Zero disables both; prefixes and subsequences remain available.
    pub max_edits: u16,
    /// Proposes a correction only when no accepted result has zero edits.
    pub enable_correction: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_candidates: usize::MAX,
            max_edits: 2,
            enable_correction: true,
        }
    }
}
