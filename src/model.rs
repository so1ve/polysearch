pub type EntryId = u64;
pub type FieldId = u32;

/// A field's purpose, with fixed matching rules and ranking priority.
///
/// Use one of the built-in constants; roles cannot be customized.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Role {
    pub(crate) priority: u8,
    pub(crate) min_query_chars: u8,
    pub(crate) allow_substring: bool,
    pub(crate) allow_fuzzy: bool,
    pub(crate) allow_correction: bool,
}

/// The entry's main display name.
pub const PRIMARY_NAME: Role = Role {
    priority: 0,
    min_query_chars: 1,
    allow_substring: true,
    allow_fuzzy: true,
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
    allow_fuzzy: true,
    allow_correction: false,
};

/// A technical identifier, matched by prefixes and whole-token correction.
pub const IDENTIFIER: Role = Role {
    priority: 5,
    min_query_chars: 3,
    allow_substring: false,
    allow_fuzzy: false,
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
