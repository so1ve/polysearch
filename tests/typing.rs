use std::cmp::Ordering;

use polysearch::{ALIAS, Config, Entry, Field, KEYWORD, PRIMARY_NAME, Searcher};

const APPS: &[(&str, &str, &str)] = &[
    ("BITGATEWAY", "", "Campus network authentication"),
    ("algermusicplayer", "Alger Music", "Music player"),
    ("Firefox", "Mozilla", "Web browser"),
    ("Google Chrome", "Chromium Browser", "Web browser"),
    ("Thunderbird", "Email Client", "Mail and chat"),
    ("Visual Studio Code", "VSCode", "Code editor"),
    ("Zed", "Text Editor", "Code editor"),
    ("Oopz", "", "Voice chat"),
    ("Fcitx 5", "Input Method", "Input method configuration"),
    ("AutoSlides", "", "Course slides"),
    ("LibreOffice Writer", "Word Processor", "Write documents"),
    ("LibreOffice Calc", "Spreadsheet", "Edit spreadsheets"),
    (
        "GNOME Calculator",
        "Calculator",
        "Arithmetic and calculations",
    ),
    ("Files", "Nautilus", "File manager"),
    ("Ghostty", "Terminal", "Terminal emulator"),
    ("Kitty", "Terminal", "Terminal emulator"),
    ("KeePassXC", "Password Manager", "Manage passwords"),
    ("Telegram", "Messenger", "Instant messaging"),
    ("Element", "Matrix", "Team communication"),
    ("OBS Studio", "Screen Recorder", "Streaming and recording"),
    ("Kdenlive", "Video Editor", "Edit videos"),
    ("Audacity", "Audio Editor", "Edit sound recordings"),
    ("Blender", "3D Modeling", "Create 3D graphics"),
    ("Obsidian", "Note Taking", "Personal notes"),
    ("Logseq", "Outliner", "Personal knowledge base"),
    ("KDE Connect SMS", "", "Read and send SMS messages"),
    ("Wine Windows Program Loader", "", "Run Windows programs"),
    ("Waydroid", "Android", "Android applications"),
    ("ChatGPT Community", "ChatGPT", "Community chat client"),
    ("Kelivo", "", "A Flutter LLM chat client"),
    ("文件", "资源管理器", "文件夹和磁盘"),
    ("微信", "WeChat", "即时通讯"),
    ("音乐播放器", "MusicBox", "音频文件管理"),
    ("文档助手 Beta", "Document Helper", "文本和文档"),
    ("云·笔记", "Cloud Notes", "笔记同步"),
    ("银行助手", "Finance", "财务管理"),
];

fn launcher(config: Config) -> Searcher {
    Searcher::new(
        APPS.iter()
            .enumerate()
            .map(|(index, &(name, alias, description))| Entry {
                id: index as u64,
                fields: [(PRIMARY_NAME, name), (ALIAS, alias), (KEYWORD, description)]
                    .into_iter()
                    .enumerate()
                    .map(|(field, (role, text))| Field {
                        id: (index * 3 + field) as u32,
                        role,
                        text: text.into(),
                    })
                    .collect(),
            }),
        config,
    )
}

fn matches(searcher: &Searcher, query: &str) -> Vec<&'static str> {
    searcher
        .search(query, APPS.len(), |_, _| Ordering::Equal)
        .into_iter()
        .map(|result| APPS[result.entry as usize].0)
        .collect()
}

fn assert_matches<'a>(
    searcher: &Searcher,
    cases: impl IntoIterator<Item = (&'a str, &'a str)>,
    limit: usize,
) {
    let mut failures = Vec::new();

    for (query, expected) in cases {
        let results = matches(searcher, query);

        if !results.iter().take(limit).any(|name| *name == expected) {
            failures.push(format!("{query:?} -> {expected}: {results:?}"));
        }
    }

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn common_typos_and_aliases() {
    let cases = [
        ("gaetway", "BITGATEWAY"),
        ("gaet", "BITGATEWAY"),
        ("gatway", "BITGATEWAY"),
        ("gaateway", "BITGATEWAY"),
        ("gatewsy", "BITGATEWAY"),
        ("gateawy", "BITGATEWAY"),
        ("qateway", "BITGATEWAY"),
        ("gatewayx", "BITGATEWAY"),
        ("algormusic", "algermusicplayer"),
        ("algo", "algermusicplayer"),
        ("muisc", "algermusicplayer"),
        ("palyer", "algermusicplayer"),
        ("fireofx", "Firefox"),
        ("firfox", "Firefox"),
        ("fireffox", "Firefox"),
        ("gogle", "Google Chrome"),
        ("chorme", "Google Chrome"),
        ("thudnerbird", "Thunderbird"),
        ("thunderbrd", "Thunderbird"),
        ("thunderbrid", "Thunderbird"),
        ("vscode", "Visual Studio Code"),
        ("vsc", "Visual Studio Code"),
        ("vscd", "Visual Studio Code"),
        ("vs code", "Visual Studio Code"),
        ("vs-code", "Visual Studio Code"),
        ("VS_CODE", "Visual Studio Code"),
        ("gchrome", "Google Chrome"),
        ("gc", "Google Chrome"),
        ("lw", "LibreOffice Writer"),
        ("loc", "LibreOffice Calc"),
        ("stduio", "Visual Studio Code"),
        ("studio cdoe", "Visual Studio Code"),
        ("zd", "Zed"),
        ("zde", "Zed"),
        ("zedd", "Zed"),
        ("op", "Oopz"),
        ("oozp", "Oopz"),
        ("fcitix", "Fcitx 5"),
        ("fic tx", "Fcitx 5"),
        ("slieds", "AutoSlides"),
        ("wirter", "LibreOffice Writer"),
        ("libreofice", "LibreOffice Writer"),
        ("calcluator", "GNOME Calculator"),
        ("clac", "GNOME Calculator"),
        ("ghsotty", "Ghostty"),
        ("keeapss", "KeePassXC"),
        ("telegrma", "Telegram"),
        ("obsstduio", "OBS Studio"),
        ("kdenilve", "Kdenlive"),
        ("audactiy", "Audacity"),
        ("blneder", "Blender"),
        ("obsidain", "Obsidian"),
        ("logsqe", "Logseq"),
        ("mozilla", "Firefox"),
        ("mozila", "Firefox"),
        ("nautilus", "Files"),
        ("nautlius", "Files"),
        ("word processor", "LibreOffice Writer"),
        ("word procesor", "LibreOffice Writer"),
        ("spreadsheet", "LibreOffice Calc"),
        ("spreadshet", "LibreOffice Calc"),
        ("password manager", "KeePassXC"),
        ("password manger", "KeePassXC"),
        ("screen recorder", "OBS Studio"),
        ("screen recroder", "OBS Studio"),
        ("matrix", "Element"),
        ("matirx", "Element"),
        ("outliner", "Logseq"),
        ("outlienr", "Logseq"),
        ("file manager", "Files"),
        ("File-Manager", "Files"),
        ("wechat", "微信"),
        ("wecaht", "微信"),
        ("musicbox", "音乐播放器"),
        ("muiscbox", "音乐播放器"),
        ("资源管理器", "文件"),
        ("文档助手", "文档助手 Beta"),
        ("助手 beta", "文档助手 Beta"),
        ("云 笔记", "云·笔记"),
    ];

    for enable_correction in [false, true] {
        let searcher = launcher(Config {
            enable_correction,
            ..Config::default()
        });

        assert_matches(&searcher, cases, 3);
    }
}

#[test]
fn unrelated_fragments_do_not_become_matches() {
    let searcher = launcher(Config::default());

    for (query, rejected) in [
        ("amd", "KDE Connect SMS"),
        ("amd", "Wine Windows Program Loader"),
        ("oz", "Oopz"),
        ("wyyland", "Waydroid"),
        ("gatexyz", "BITGATEWAY"),
        ("zzgatewayzz", "BITGATEWAY"),
        ("firechrome", "Firefox"),
        ("firechrome", "Google Chrome"),
        ("calcwriter", "LibreOffice Writer"),
    ] {
        assert!(
            !matches(&searcher, query).contains(&rejected),
            "{query:?} -> {rejected}"
        );
    }
}

#[test]
fn single_edit_patterns_across_names_and_aliases() {
    let seeds = [
        ("bitgateway", "BITGATEWAY"),
        ("gateway", "BITGATEWAY"),
        ("algermusicplayer", "algermusicplayer"),
        ("music", "algermusicplayer"),
        ("player", "algermusicplayer"),
        ("firefox", "Firefox"),
        ("mozilla", "Firefox"),
        ("google", "Google Chrome"),
        ("chrome", "Google Chrome"),
        ("thunderbird", "Thunderbird"),
        ("visual", "Visual Studio Code"),
        ("studio", "Visual Studio Code"),
        ("studiocode", "Visual Studio Code"),
        ("vscode", "Visual Studio Code"),
        ("fcitx", "Fcitx 5"),
        ("slides", "AutoSlides"),
        ("libreoffice", "LibreOffice Writer"),
        ("writer", "LibreOffice Writer"),
        ("wordprocessor", "LibreOffice Writer"),
        ("spreadsheet", "LibreOffice Calc"),
        ("calculator", "GNOME Calculator"),
        ("nautilus", "Files"),
        ("ghostty", "Ghostty"),
        ("keepass", "KeePassXC"),
        ("passwordmanager", "KeePassXC"),
        ("telegram", "Telegram"),
        ("matrix", "Element"),
        ("screenrecorder", "OBS Studio"),
        ("kdenlive", "Kdenlive"),
        ("audacity", "Audacity"),
        ("blender", "Blender"),
        ("obsidian", "Obsidian"),
        ("logseq", "Logseq"),
        ("outliner", "Logseq"),
        ("waydroid", "Waydroid"),
        ("chatgpt", "ChatGPT Community"),
    ];
    let mut cases = std::collections::BTreeSet::new();

    for (seed, expected) in seeds {
        for (index, ch) in seed.bytes().enumerate() {
            let mut missing = seed.to_owned();
            missing.remove(index);
            cases.insert((missing, expected));

            let mut repeated = seed.to_owned();
            repeated.insert(index, char::from(ch));
            cases.insert((repeated, expected));

            let row = ["qwertyuiop", "asdfghjkl", "zxcvbnm"]
                .into_iter()
                .find(|row| row.contains(char::from(ch)))
                .unwrap();
            let key = row.find(char::from(ch)).unwrap();
            let neighbor = if key == 0 { 1 } else { key - 1 };
            let mut mistyped = seed.as_bytes().to_vec();
            mistyped[index] = row.as_bytes()[neighbor];
            cases.insert((String::from_utf8(mistyped).unwrap(), expected));

            if index + 1 < seed.len() && seed.as_bytes()[index + 1] != ch {
                let mut swapped = seed.as_bytes().to_vec();
                swapped.swap(index, index + 1);
                cases.insert((String::from_utf8(swapped).unwrap(), expected));
            }
        }
    }

    for enable_correction in [false, true] {
        let searcher = launcher(Config {
            enable_correction,
            ..Config::default()
        });
        let cases = cases
            .iter()
            .map(|(query, expected)| (query.as_str(), *expected));

        assert_matches(&searcher, cases, 5);
    }
}

#[test]
fn misspelled_fragments_remain_available_while_typing() {
    let searcher = launcher(Config::default());
    let inputs = [
        ("gaetway", "BITGATEWAY"),
        ("gatewsy", "BITGATEWAY"),
        ("muiscplayer", "algermusicplayer"),
        ("stduiocode", "Visual Studio Code"),
        ("algormusicplayer", "algermusicplayer"),
        ("claculator", "GNOME Calculator"),
        ("fireofx", "Firefox"),
    ];
    let cases = inputs
        .into_iter()
        .flat_map(|(input, expected)| (3..=input.len()).map(move |end| (&input[..end], expected)));

    assert_matches(&searcher, cases, 5);
}

#[test]
fn internal_typos_respect_the_edit_budget() {
    for max_edits in [0, 1, 2] {
        let searcher = launcher(Config {
            max_edits,
            ..Config::default()
        });

        assert!(matches(&searcher, "gateway").contains(&"BITGATEWAY"));
        assert_eq!(
            matches(&searcher, "gaetway").contains(&"BITGATEWAY"),
            max_edits >= 1
        );
        assert_eq!(
            matches(&searcher, "gaetwsy").contains(&"BITGATEWAY"),
            max_edits >= 2
        );
        assert!(!matches(&searcher, "gaetzsy").contains(&"BITGATEWAY"));
    }
}

#[test]
#[cfg(feature = "pinyin")]
fn pinyin_names_initials_and_typos() {
    let searcher = launcher(Config::default());
    let cases = [
        ("weixin", "微信"),
        ("wx", "微信"),
        ("wiexin", "微信"),
        ("weixni", "微信"),
        ("weixiin", "微信"),
        ("wenjian", "文件"),
        ("wenjiam", "文件"),
        ("wj", "文件"),
        ("ziyuanguanliqi", "文件"),
        ("ziyuagualiqi", "文件"),
        ("zyglq", "文件"),
        ("yinyuebofangqi", "音乐播放器"),
        ("yinyuebofagnqi", "音乐播放器"),
        ("yybfq", "音乐播放器"),
        ("wendangzhushou", "文档助手 Beta"),
        ("wnedang", "文档助手 Beta"),
        ("wendagnzhushou", "文档助手 Beta"),
        ("wdzs", "文档助手 Beta"),
        ("wdzsbeta", "文档助手 Beta"),
        ("yunbiji", "云·笔记"),
        ("ybj", "云·笔记"),
        ("biji", "云·笔记"),
        ("yunbji", "云·笔记"),
        ("yinhang", "银行助手"),
        ("yinxing", "银行助手"),
    ];

    assert_matches(&searcher, cases, 3);

    let inputs = [
        ("wiexin", "微信"),
        ("wnedangzhushou", "文档助手 Beta"),
        ("wenjiam", "文件"),
    ];
    let cases = inputs
        .into_iter()
        .flat_map(|(input, expected)| (3..=input.len()).map(move |end| (&input[..end], expected)));

    assert_matches(&searcher, cases, 5);
}
