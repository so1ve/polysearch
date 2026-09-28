use crate::text::NormalizedText;

const MAX_FIELD_CHARS: usize = 4_096;

pub struct Term {
    pub text: NormalizedText,
    pub penalty: u16,
}

pub fn expand(input: &str) -> Vec<Term> {
    let mut terms: Vec<Term> = Vec::new();
    let mut remaining_chars = MAX_FIELD_CHARS;
    let mut add = |input: &str, penalty: u16| {
        let text = NormalizedText::new(input);

        if text.chars.is_empty() {
            return;
        }

        if let Some(existing) = terms.iter_mut().find(|term| term.text.chars == text.chars) {
            existing.penalty = existing.penalty.min(penalty);
        } else if text.chars.len() <= remaining_chars {
            remaining_chars -= text.chars.len();
            terms.push(Term { text, penalty });
        }
    };

    add(input, 0);

    #[cfg(feature = "pinyin")]
    super::pinyin::expand(input, add);

    terms.sort_by(|left, right| {
        left.penalty
            .cmp(&right.penalty)
            .then_with(|| left.text.text.cmp(&right.text.text))
    });

    terms
}
