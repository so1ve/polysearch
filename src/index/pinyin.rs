use pinyin_pro::get_all_pinyin;
use pinyin_pro::options::{
    NonZh, PatternKind, PinyinOptions, PinyinOutput, SurnameMode, ToneType, TypeMode, VMode,
};
use pinyin_pro::pinyin::pinyin;
use pinyin_pro::pinyin_utils::strip_tone;

use super::Kind;

const MAX_READINGS: usize = 8;

pub fn expand(input: &str, mut emit: impl FnMut(&str, Kind)) {
    if !input.chars().any(is_han) {
        return;
    }

    let read = |pattern| {
        let options = PinyinOptions {
            pattern,
            tone_type: ToneType::None,
            type_mode: TypeMode::Str,
            v: VMode::V,
            non_zh: NonZh::Consecutive,
            traditional: true,
            ..PinyinOptions::default()
        };

        let PinyinOutput::Str(value) = pinyin(input, options) else {
            unreachable!();
        };

        value
    };

    emit(&read(PatternKind::Pinyin), Kind::Pinyin);

    let alternatives = AlternativeReadings::new(input);

    for reading in alternatives.full {
        emit(&reading, Kind::Pinyin);
    }

    emit(&read(PatternKind::First), Kind::Initials);

    for reading in alternatives.initials {
        emit(&reading, Kind::Initials);
    }
}

struct AlternativeReadings {
    full: Vec<String>,
    initials: Vec<String>,
}

impl AlternativeReadings {
    fn new(input: &str) -> Self {
        let mut readings = Self {
            full: vec![String::new()],
            initials: vec![String::new()],
        };

        for ch in input.chars() {
            if !is_han(ch) {
                for value in readings.full.iter_mut().chain(&mut readings.initials) {
                    value.push(ch);
                }

                continue;
            }

            let mut options: Vec<_> = get_all_pinyin(&ch.to_string(), SurnameMode::Off)
                .into_iter()
                .map(|reading| strip_tone(&reading).replace('ü', "v"))
                .collect();
            options.sort();
            options.dedup();

            if options.is_empty() {
                options.push(ch.to_string());
            }

            let mut initials: Vec<String> = options
                .iter()
                .map(|reading| reading.chars().take(1).collect())
                .collect();
            initials.dedup();

            // Pinyin spellings are indexed without spaces.
            for (variants, options) in [
                (&mut readings.full, &options),
                (&mut readings.initials, &initials),
            ] {
                *variants = variants
                    .iter()
                    .flat_map(|prefix| {
                        options.iter().map(move |option| {
                            let mut value = String::with_capacity(prefix.len() + option.len());
                            value.push_str(prefix);
                            value.push_str(option);

                            value
                        })
                    })
                    .take(MAX_READINGS - 1)
                    .collect();
            }
        }

        readings
    }
}

const fn is_han(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0x20000..=0x2FA1F
    )
}

#[cfg(test)]
mod tests {
    use crate::test_support::{entries, entry, result_ids};
    use crate::{Config, KEYWORD, PRIMARY_NAME, Searcher};

    #[test]
    fn literal_names_and_full_readings_outrank_lossy_initials() {
        let searcher = Searcher::new(
            entries(&["bj", "笔记", "bjtool", "bijitool"]),
            Config::default(),
        );

        assert_eq!(result_ids(&searcher, "bj", 10), [1, 3, 2, 4]);
        assert_eq!(
            searcher.search("biji", 1, |id| if id == 4 { 255 } else { 0 })[0].entry,
            2
        );
    }

    #[test]
    fn full_pinyin_keywords_outrank_fuzzy_names() {
        let searcher = Searcher::new(
            [
                entry(
                    1,
                    vec![(1, PRIMARY_NAME, "thinking face"), (2, KEYWORD, "思考")],
                ),
                entry(2, vec![(3, PRIMARY_NAME, "司空")]),
                entry(3, vec![(4, PRIMARY_NAME, "石刻")]),
            ],
            Config::default(),
        );

        assert_eq!(result_ids(&searcher, "sikao", 1), [1]);
    }

    #[test]
    fn initials_support_typos_and_skipped_characters() {
        let searcher = Searcher::new(entries(&["文件资源管理器"]), Config::default());

        for query in ["wjzyglq", "wjzyglx", "wzglq"] {
            assert_eq!(result_ids(&searcher, query, 1), [1], "query={query}");
        }
    }
}
