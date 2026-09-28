pub struct NormalizedText {
    pub text: String,
    pub chars: Vec<char>,
}

impl NormalizedText {
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

    pub fn match_key(&self) -> String {
        self.chars.iter().collect()
    }
}
