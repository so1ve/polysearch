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
