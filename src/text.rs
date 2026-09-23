pub struct NormalizedText {
    text: String,
    chars: Vec<char>,
}

impl NormalizedText {
    #[must_use]
    pub fn new(input: &str) -> Self {
        let mut text = String::with_capacity(input.len());
        let mut chars = Vec::new();
        let mut pending_space = false;

        for character in input.chars() {
            if character.is_whitespace() || matches!(character, '/' | '\\' | '_' | '-' | '.' | '·')
            {
                if !text.is_empty() {
                    pending_space = true;
                }

                continue;
            }

            if pending_space {
                text.push(' ');
                pending_space = false;
            }

            for lower in character.to_lowercase() {
                text.push(lower);
                chars.push(lower);
            }
        }

        Self { text, chars }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn chars(&self) -> &[char] {
        &self.chars
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.chars.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    #[must_use]
    pub fn match_key(&self) -> String {
        self.chars.iter().collect()
    }
}
