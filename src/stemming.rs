//! stemming.rs

use std::borrow::Cow;

use rust_stemmers::{Algorithm, Stemmer};
use std::collections::HashSet;

pub fn stemmer<'a>(tokenized: &HashSet<&'a str>) -> HashSet<Cow<'a, str>> {
    // Initialize the stemmer for your target language
    let make_stemmer = Stemmer::create(Algorithm::English);

    // Pass all the words in the HashSet through the
    // stemmer and replace them with the stemmed version
    let mut set: HashSet<Cow<'a, str>> = HashSet::new();

    for element in tokenized.clone() {
        set.insert(make_stemmer.stem(element));
    }
    set
}

// TODO: Understand lifetimes and settle the problems here.
pub fn stemmer_hashset<'a>(tokens: HashSet<&'a str>) -> HashSet<&str> {
    let hash_set_cow: HashSet<Cow<'a, str>> = stemmer(&'a tokens);

    let hash_set_str: HashSet<&'a str> = hash_set_cow
        .iter()
        .map(|element| element.as_ref())
        .collect();
    hash_set_str
}
