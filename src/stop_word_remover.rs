//! stop_word_remover
use crate::text_processing::text_processor;
use std::{collections::HashSet, fs};

// Bring in the stop_words
pub fn get_stop_words() -> String {
    let stop_words = fs::read_to_string("stop_words.txt").expect("Failed to read from file!");
    let cleaned_stop_words: String = text_processor(&stop_words);
    cleaned_stop_words
}
// Store them in a HashSet
// not using vec![] since it provides many useful functionalities
// viz. No Duplicates, Operations are faster interms of Big O
//
// preprocess the stop word HashSet

pub fn remover_stop_words(mut token: HashSet<&str>) -> HashSet<&str> {
    let stop_words: String = get_stop_words();

    // Convert the stop words to `HashSet<&str>`
    let stop_word_tokens: HashSet<&str> = stop_words.lines().map(|line| line.trim()).collect();

    // Remove stop words from `HashSet<&str>`
    token.retain(|item| !stop_word_tokens.contains(item));

    token
}
