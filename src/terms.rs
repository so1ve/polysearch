use crate::text::NormalizedText;

pub struct Term {
    pub text: NormalizedText,
    pub cost: u16,
}

/// A small set of distinct terms, bounded by count and total characters.
pub struct Terms {
    pub values: Vec<Term>,
    max_terms: usize,
    remaining_chars: usize,
}

impl Terms {
    #[must_use]
    pub fn new(input: &str, max_terms: usize, max_chars: usize) -> Self {
        let mut terms = Self {
            values: Vec::new(),
            max_terms,
            remaining_chars: max_chars,
        };
        terms.add(input, 0);

        terms
    }

    pub fn add(&mut self, input: &str, cost: u16) {
        let text = NormalizedText::new(input);

        if text.is_empty() {
            return;
        }

        if let Some(existing) = self
            .values
            .iter_mut()
            .find(|term| term.text.chars() == text.chars())
        {
            existing.cost = existing.cost.min(cost);
        } else {
            if self.values.len() == self.max_terms || text.len() > self.remaining_chars {
                return;
            }

            self.remaining_chars -= text.len();
            self.values.push(Term { text, cost });
        }

        self.values.sort_by(|left, right| {
            left.cost
                .cmp(&right.cost)
                .then_with(|| left.text.as_str().cmp(right.text.as_str()))
        });
    }
}
