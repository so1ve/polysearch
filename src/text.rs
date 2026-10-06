pub fn normalize(input: &str) -> String {
    let mut text = String::with_capacity(input.len());
    let mut space = false;
    let mut previous = None;
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch.is_whitespace() || matches!(ch, '/' | '\\' | '_' | '-' | '.' | '·') {
            space = !text.is_empty();
            previous = None;
            continue;
        }

        // Preserve word boundaries before folding case: AutoSlides, ChatGPT.
        if ch.is_uppercase()
            && previous.is_some_and(|prev: char| {
                prev.is_lowercase()
                    || (prev.is_uppercase() && chars.peek().is_some_and(|next| next.is_lowercase()))
            })
        {
            space = true;
        }

        if space {
            text.push(' ');
            space = false;
        }

        text.extend(ch.to_lowercase());
        previous = Some(ch);
    }

    text
}
