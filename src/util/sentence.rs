/// `text` with its first letter in lower case, for a label placed mid-sentence.
pub fn lowercase_first(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_lowercase().chain(chars).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_first_letter_changes() {
        assert_eq!(lowercase_first("Formatting SanDisk"), "formatting SanDisk");
        assert_eq!(lowercase_first(""), "");
    }
}
