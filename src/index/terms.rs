use super::Kind;
use crate::text::normalize;

pub fn expand(input: &str, mut emit: impl FnMut(String, Kind)) {
    let mut add = |text: String, kind| {
        if !text.is_empty() {
            emit(text, kind);
        }
    };

    let text = normalize(input);
    if text.contains(' ') {
        add(text.replace(' ', ""), Kind::Literal);
    }
    add(text, Kind::Literal);

    #[cfg(feature = "pinyin")]
    super::pinyin::expand(input, |reading, kind| {
        add(normalize(reading).replace(' ', ""), kind);
    });
}
