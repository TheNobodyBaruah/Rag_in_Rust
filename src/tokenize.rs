//! tokenize.rs

use std::collections::HashSet;

pub fn tokenizer(text: &String) -> HashSet<&str> {
    // `name` should be split into white spaces
    let tokenied_movie_names: HashSet<&str> = text.split_whitespace().collect();
    tokenied_movie_names
}

// UNTI TEST, not written yet!
