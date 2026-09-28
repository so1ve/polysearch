use pinyin_pro::get_all_pinyin;
use pinyin_pro::options::{
    NonZh, PatternKind, PinyinOptions, PinyinOutput, SurnameMode, ToneType, TypeMode, VMode,
};
use pinyin_pro::pinyin::pinyin;
use pinyin_pro::pinyin_utils::strip_tone;

const FULL_PINYIN_PENALTY: u16 = 18;
const INITIALS_PENALTY: u16 = 28;
const ALTERNATIVE_PINYIN_PENALTY: u16 = 22;
const ALTERNATIVE_INITIALS_PENALTY: u16 = 32;
const MAX_READINGS: usize = 8;

pub fn expand(input: &str, mut emit: impl FnMut(&str, u16)) {
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

    emit(&read(PatternKind::Pinyin), FULL_PINYIN_PENALTY);

    let alternatives = AlternativeReadings::new(input);

    for reading in alternatives.full {
        emit(&reading, ALTERNATIVE_PINYIN_PENALTY);
    }

    emit(&read(PatternKind::First), INITIALS_PENALTY);

    for reading in alternatives.initials {
        emit(&reading, ALTERNATIVE_INITIALS_PENALTY);
    }
}

struct AlternativeReadings {
    full: Vec<String>,
    initials: Vec<String>,
    previous_han: bool,
}

impl AlternativeReadings {
    fn new(input: &str) -> Self {
        let mut readings = Self {
            full: vec![String::new()],
            initials: vec![String::new()],
            previous_han: false,
        };

        for ch in input.chars() {
            readings.push(ch);
        }

        readings
    }

    fn push(&mut self, ch: char) {
        let han = is_han(ch);
        let mut options = if han {
            get_all_pinyin(&ch.to_string(), SurnameMode::Off)
                .into_iter()
                .map(|reading| strip_tone(&reading).replace('ü', "v"))
                .collect::<Vec<_>>()
        } else {
            vec![ch.to_string()]
        };

        options.sort();
        options.dedup();

        if options.is_empty() {
            options.push(ch.to_string());
        }

        let mut initials = if han {
            options
                .iter()
                .map(|reading| reading.chars().take(1).collect())
                .collect::<Vec<String>>()
        } else {
            options.clone()
        };
        initials.sort();
        initials.dedup();

        for (variants, options) in [(&mut self.full, &options), (&mut self.initials, &initials)] {
            let mut next = Vec::new();

            'variants: for prefix in variants.iter() {
                for option in options {
                    let mut value = String::with_capacity(prefix.len() + option.len() + 1);
                    value.push_str(prefix);

                    if !prefix.is_empty() && (self.previous_han || han) {
                        value.push(' ');
                    }

                    value.push_str(option);

                    if !next.contains(&value) {
                        next.push(value);
                    }

                    if next.len() >= MAX_READINGS - 1 {
                        break 'variants;
                    }
                }
            }

            *variants = next;
        }

        self.previous_han = han;
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
    use crate::test_support::{entries, result_ids};
    use crate::{Config, Searcher};

    #[test]
    fn pinyin_uses_the_same_tiers_as_literal_text() {
        let searcher = Searcher::new(
            entries(&["bj", "笔记", "bjtool", "bijitool"]),
            Config::default(),
        );

        assert_eq!(result_ids(&searcher, "bj", 10), [1, 2, 3, 4]);

        let results = searcher.search("bj", 10, |left, right| right.cmp(&left));

        assert_eq!(
            results
                .iter()
                .map(|result| result.entry)
                .collect::<Vec<_>>(),
            [2, 1, 3, 4]
        );
        assert_eq!(
            searcher.search("biji", 1, |left, right| right.cmp(&left))[0].entry,
            2
        );
    }

    #[test]
    fn alternative_readings_are_searchable() {
        let searcher = Searcher::new(
            entries(&["银行"]),
            Config {
                max_edits: 0,
                enable_correction: false,
                ..Config::default()
            },
        );

        for query in ["yinhang", "yinxing"] {
            assert_eq!(result_ids(&searcher, query, 5), [1], "query={query}");
        }
    }

    #[test]
    fn mixed_names_preserve_non_chinese_content() {
        let searcher = Searcher::new(
            entries(&["文档助手 Beta"]),
            Config {
                max_edits: 0,
                enable_correction: false,
                ..Config::default()
            },
        );

        for query in ["wendangzhushoubeta", "wdzsbeta"] {
            assert_eq!(result_ids(&searcher, query, 5), [1], "query={query}");
        }
    }
}
