use crate::{Entry, EntryId, Field, FieldId, PRIMARY_NAME, Role, Searcher};

pub fn entry(id: EntryId, fields: Vec<(FieldId, Role, &str)>) -> Entry {
    Entry {
        id,
        fields: fields
            .into_iter()
            .map(|(field_id, role, text)| Field {
                id: field_id,
                role,
                text: text.into(),
            })
            .collect(),
    }
}

pub fn entries(names: &[&str]) -> Vec<Entry> {
    names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            entry(
                index as EntryId + 1,
                vec![(index as FieldId + 1, PRIMARY_NAME, name)],
            )
        })
        .collect()
}

pub fn result_ids(searcher: &Searcher, query: &str, limit: usize) -> Vec<EntryId> {
    searcher
        .search(query, limit)
        .into_iter()
        .map(|result| result.entry)
        .collect()
}
