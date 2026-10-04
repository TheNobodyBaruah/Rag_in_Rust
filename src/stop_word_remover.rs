//! stop_word_remover
use crate::text_processing::text_processor;
use std::fs;

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
