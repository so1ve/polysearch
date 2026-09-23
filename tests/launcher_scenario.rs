use polysearch::{
    ALIAS, Config, Entry, EntryId, Field, FieldId, IDENTIFIER, KEYWORD, PRIMARY_NAME, Searcher,
};

fn app(id: EntryId, name: &str, alias: &str, keyword: &str, identifier: &str) -> Entry {
    let fields = [
        (PRIMARY_NAME, name),
        (ALIAS, alias),
        (KEYWORD, keyword),
        (IDENTIFIER, identifier),
    ]
    .into_iter()
    .enumerate()
    .map(|(offset, (role, text))| Field {
        id: id as FieldId * 10 + offset as FieldId,
        role,
        text: text.into(),
    })
    .collect();

    Entry { id, fields }
}

fn launcher() -> Searcher {
    let entries = [
        app(
            1,
            "Files",
            "file manager",
            "documents",
            "org.gnome.Nautilus",
        ),
        app(2, "Ghostty", "terminal", "shell", "com.mitchellh.ghostty"),
        app(3, "终端", "terminal", "shell", "org.gnome.Terminal"),
        app(4, "云·笔记", "云笔记", "笔记同步", "app.cloud.notes"),
        app(5, "Zed", "", "", "dev.zed.Zed"),
    ];

    Searcher::new(entries, Config::default())
}

#[test]
fn desktop_fields_select_ranked_unique_applications() {
    let searcher = launcher();

    for (query, expected) in [
        ("file manager", vec![(1, 11)]),
        ("documents", vec![(1, 12)]),
        ("notse", vec![(4, 43)]),
        ("terminal", vec![(2, 21), (3, 31)]),
    ] {
        let matches: Vec<_> = searcher
            .search(query, 10)
            .into_iter()
            .map(|result| (result.entry, result.field))
            .collect();

        assert_eq!(matches, expected, "query={query}");
    }
}

#[test]
fn short_substrings_work_across_languages_and_respect_field_roles() {
    let searcher = launcher();

    for (query, expected) in [
        ("en", vec![]),
        ("笔", vec![(4, 40)]),
        ("笔记", vec![(4, 40)]),
        ("il", vec![(1, 10)]),
        ("ile", vec![(1, 10)]),
    ] {
        let matches: Vec<_> = searcher
            .search(query, 10)
            .into_iter()
            .map(|result| (result.entry, result.field))
            .collect();

        assert_eq!(matches, expected, "query={query}");
    }
}

#[test]
#[cfg(feature = "pinyin")]
fn pinyin_queries_keep_the_name_field() {
    let searcher = launcher();

    for (query, expected) in [
        ("yunbiji", vec![(4, 40)]),
        ("ybj", vec![(4, 40)]),
        ("bj", vec![(4, 40)]),
        ("zed", vec![(5, 50)]),
    ] {
        let matches: Vec<_> = searcher
            .search(query, 10)
            .into_iter()
            .map(|result| (result.entry, result.field))
            .collect();

        assert_eq!(matches, expected, "query={query}");
    }
}
