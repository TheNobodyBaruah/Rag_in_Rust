pub fn text_processor(text: &String) -> String {
    let cleaned_text = text
        .replace(|c: char| c.is_ascii_punctuation(), "")
        .to_lowercase();
    cleaned_text
}
