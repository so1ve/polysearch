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
//! let results = searcher.search("文档", 10);
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

mod config;
mod correction;
mod distance;
mod index;
mod model;
#[cfg(feature = "pinyin")]
mod pinyin;
mod search;
mod terms;
#[cfg(test)]
mod test_support;
mod text;

pub use config::Config;
pub use model::{
    ALIAS, Entry, EntryId, Field, FieldId, IDENTIFIER, KEYWORD, LOCALIZED_NAME, PRIMARY_NAME, Role,
};
pub use search::{SearchResult, Searcher};
