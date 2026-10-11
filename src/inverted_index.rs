//! src/inverted_index.rs

use crate::stemming::stemmer;
use crate::stop_word_remover::{self, get_stop_words, remover_stop_words};
use crate::text_processing::text_processor;
use crate::tokenize::{self, tokenizer};
use crate::{Movie, data_loader::json_loader};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fmt::format;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};

// Note: Add `bincode` to Cargo.toml dependencies if using bincode serialization.
pub struct InvertedIndex {
    pub index: HashMap<String, Vec<u32>>, // mapping tokens to doc_id
    pub docmap: HashMap<u32, Movie>,      // mapping doc_id to full docs
    pub block_size_limit: usize,          // docs to process b4 flusing to disk
    pub block_counter: u32,               // blocks written to RAM
    pub docs_in_current_batch: usize,     // no of indexed docs in current block
}

impl InvertedIndex {
    // Populates the `index` field of the struct (in memory-HashMap)
    // Also tracks size of HashMap flusing data to a file on disk
    // once `block_size_limit` reached, prevents running out of RAM
    pub fn add_document(&mut self, doc_id: u32, text: &str, stop_word_tokens: &HashSet<&str>) {
        // 1. Text Normalization Pipeline (must match query preprocessing):
        let text: String = text.to_string();
        //    - Clean text: lowercase and strip ASCII punctuation (using text_processor)
        let cleaned_text = text_processor(&text);
        //    - Tokenize text by whitespace into tokens (using tokenizer)
        let tokenized_text = tokenizer(&cleaned_text);
        //    - Filter out stop words (using get_stop_words)
        let tokens_without_stop_words: HashSet<&str> = tokenized_text
            .into_iter()
            .filter(|movie_name| !stop_word_tokens.contains(movie_name))
            .collect();

        println!("tokens_without_stop_words: {:?}", tokens_without_stop_words);

        //    - Stem the remaining tokens (using stemmer)
        let stemmed_token_cow: HashSet<std::borrow::Cow<'_, str>> =
            stemmer(&tokens_without_stop_words);
        let stemmed_tokens: HashSet<&str> = stemmed_token_cow
            .iter()
            .map(|token| token.as_ref())
            .collect();

        // 2. Add tokens to the in-memory inverted index:
        //    - Deduplicate tokens per document (e.g., using a HashSet) so doc_id
        //      is not repeatedly added to the posting list if a term appears multiple times.
        //    - For each unique stemmed token, insert/append `doc_id` to `self.index`.
        /*  This is something to think about, this method is called after an instance of the struct
        created, otherwise it cannot be called at all, therefore index cannot be empty, which
        means an empty `HashMap` is created when the struct is instantiated */
        stemmed_tokens.into_iter().for_each(|stemmed_token| {
            self.index
                .entry(stemmed_token.to_string())
                .or_insert_with(Vec::new)
                .push(doc_id);
        });

        // 3. Check memory / block threshold:
        //    - Increment `self.docs_in_current_batch`.
        self.docs_in_current_batch += 1;
        //    - Check if `self.docs_in_current_batch >= self.block_size_limit`.
        //    - If it does, call `self.write_block()` to flush the in-memory index to disk and reset RAM.
        if self.docs_in_current_batch >= self.block_size_limit {
            self.write_block();
        }
    }

    // alphabetically sorts token in `index` -> serializes them to
    // a block file in disk
    // clears HashMap to free up RAM
    // increments block_counter to prepare next batch of docs
    fn write_block(&mut self) {
        // This executes when the in-memory document threshold `block_size_limit` is reached.
        // Ensuring output directory exists.
        fs::create_dir_all("cache").expect("Failed to create cache directory");

        // 1. Extract all keys (tokens) from `self.index` and sort them alphabetically
        //    (SPIMI requirement so blocks can be merged sequentially).
        let mut sorted_entries: Vec<(String, Vec<u32>)> = self.index.drain().collect();
        sorted_entries.sort_by(|(token_a, _), (token_b, _)| token_a.cmp(&token_b));
        // above line can also be written as
        // sorted_entries.sort_by(|a, b| a.0.cmp(&b.0)), since a and b represent the entire tuple
        // hence a.0 and &b.0 is used.

        // 2. Serialize the sorted dictionary to a block file on disk
        //    (e.g., cache/block_{self.block_counter}.bincode or json).
        //      - Create name for the file to be created to be written on in the disk
        let block_name: String = format!("cache/block_{}.bincode", self.block_counter);
        let file = File::create(&block_name).expect("Failed to create file in the disk");
        let writer = BufWriter::new(file);
        // writing json
        bincode::serialize_into(writer, &sorted_entries)
            .expect("Failed to write json to file in disk");
        //    Ensure the `cache/` directory exists before writing.

        // 3. Clear the in-memory `self.index` HashMap to free up RAM.
        self.index.clear();

        // 4. Reset `self.docs_in_current_batch` to 0 and increment `self.block_counter`.
        //      Resetting batch state
        self.docs_in_current_batch = 0; // initialized to 0
        self.block_counter += 1;
    }

    // Reads all intermediate sorted block files in `/cache` directory and performs an
    // N-way merge
    // Creates single finalizzed inverted index file
    fn merge_blocks(&self) {
        // NOTE: This is a complicated method, everything about it is written in
        // `notes/merge_blocks_method.md` file.
        //
        // 1. Open all generated block files from the `cache/` directory.
        let mut block_iters: Vec<std::vec::IntoIter<(String, Vec<u32>)>> = Vec::new();

        for i in 0..self.block_counter {
            let block_path = format!("cache/block_{}.bincode", i);

            if let Ok(file) = File::open(&block_path) {
                let reader = BufReader::new(file);
                let entries: Vec<(String, Vec<u32>)> = bincode::deserialize_from(reader)
                    .expect("Failed to deserialize intermediate block file");
                block_iters.push(entries.into_iter());
            }
        }
        // if no blocks were written, return early
        if block_iters.is_empty() {
            return;
        }

        // 2. Perform an N-way merge across all block streams (e.g., using a Min-Heap / BinaryHeap
        //    to read terms sequentially from disk without loading all blocks into memory at once).
        //    N-way Merge across all block streams using a BinaryHeap
        //    In Rust, BinaryHeap is a Max-Heap by default, so std::cmp::Reverse is used for
        //    Min-Heap ordering.
        //    Heap stores: Reverse((term, posting_list, block_index))
        let mut min_heap: BinaryHeap<Reverse<(String, Vec<u32>, usize)>> = BinaryHeap::new();
        //    Seed the heap with the first (smallest) term for each block stream
        for (block_idx, iter) in block_iters.iter_mut().enumerate() {
            if let Some((term, postings)) = iter.next() {
                min_heap.push(Reverse((term, postings, block_idx)));
            }
        }

        let mut merged_index: HashMap<String, Vec<u32>> = HashMap::new();
        let mut current_term: Option<String> = None; // This is an allocator to use in the loop
        let mut current_postings: Vec<u32> = Vec::new(); // same as the above variable

        // 3. For identical terms across different blocks, merge and concatenate their posting lists
        //    (doc IDs will naturally be in ascending order).
        while let Some(Reverse((term, postings, block_idx))) = min_heap.pop() {
            // This returns the largest element, which is at the last, so we'll
            // have to use reverse
            if let Some(ref cur) = current_term {
                if *cur == term {
                    // Same term found in another block: append positings

                    current_postings.extend(postings);
                } else {
                    // Reached a new term: commit previous term into the merged index
                    merged_index.insert(current_term.take().unwrap(), current_postings);
                    // `.take()` method left current_term = None
                    current_term = Some(term);
                    current_postings = postings;
                }
            } else {
                current_term = Some(term);
                current_postings = postings;
            }

            // Advance the iterator for the block we just popped from and push the next entry
            if let Some((next_term, next_postings)) = block_iters[block_idx].next() {
                min_heap.push(Reverse((next_term, next_postings, block_idx)));
            }
        }

        // Commit the final term

        if let Some(term) = current_term {
            merged_index.insert(term, current_postings);
        }

        // 4. Write the final merged dictionary to a single file (cache/merged_index.bincode).
        let output_file =
            File::create("cache/merged_index.bincode").expect("Failed to create merged index file");
        let writer = BufWriter::new(output_file);
        bincode::serialize_into(writer, &merged_index)
            .expect("Failed to serialize final merged index");
    }

    // Loads the pre-built merged index and docmap from disk into memory
    // To be called in the main, such that whenever search functionality is
    // invoked the bincode is deserialized and loaded in self.index
    pub fn load_from_cache() -> Result<Self, Box<dyn std::error::Error>> {
        let index_file = File::open("cache/merged_index.bincode")?;
        let index: HashMap<String, Vec<u32>> =
            bincode::deserialize_from(BufReader::new(index_file))?;

        let docmap_file = File::open("cache/docmap.bincode")?;
        let docmap_reader = BufReader::new(docmap_file);

        // Deserialize the binary payload into the docmap (doc_id -> Movie).
        let docmap: HashMap<u32, Movie> = bincode::deserialize_from(docmap_reader)?;

        Ok(Self {
            index,
            docmap,
            block_size_limit: 0,
            block_counter: 0,
            docs_in_current_batch: 0,
        })
    }

    // looks up for preprocessed `query` tokens in finalized
    // inverted index file -> returns associated doc_ids in sorted list
    // makes it faster than keyword search
    pub fn get_documents(&self, term: &str) -> Option<&Vec<u32>> {
        // 1. Open / deserialize the finalized index (e.g., `cache/merged_index.bincode)
        //    or look up in the loaded in-memory index map.

        // 2. Query `term` in the index dictionary to get its posting list.
        //      Query is stored in HashSet, we iterate over it and get &str
        //      This method would be called per-iteration of the HashSet<&str> of the query
        //      Iterate over all the "keys" in `self.index` and get the vec of postings

        // 3. Return the associated posting list (sorted Vec<u32> of doc_ids),
        //    or an empty list / None if the term is not found in the index.
        self.index.get(term)
    }

    // calls json_loader(), and acts as the orchestrator for all the methods
    pub fn build(&mut self) {
        // 1. Call `json_loader()` to load all movies from `data/movies.json`.
        let movies: Vec<Movie> = json_loader().expect("Failed to load movies from JSON!");

        // Stop words are preloaded and tokenized once to avoid recomputing
        // per document
        let stop_words: String = get_stop_words();
        let stop_word_tokens: HashSet<&str> = tokenizer(&stop_words);

        // Ensure output cache file directory exists
        fs::create_dir_all("cache").expect("Failed to create cache directory");

        // 2. Iterate through each movie:
        for movie in movies {
            // Format text as "{title} {description}" with space in between to avoid
            // merging words
            let title_description: String = format!("{} {}", movie.title, movie.description);

            // doc_id is set to be same as movie_id in the json
            let doc_id = movie.id;

            // Pass doc_id and fomatted text through the Normalization and indexing pipeline
            self.add_document(doc_id, &title_description, &stop_word_tokens);

            // Store document in docmap (id -> Movie metadata)
            self.docmap.insert(doc_id, movie);
        }

        // 3. Once the loop finishes, flush any remaining in memory documents in the final block
        if self.docs_in_current_batch > 0 {
            self.write_block();
        }

        // 4. Merge all generated block files into cache/merged_index.bincode via N-way merge
        self.merge_blocks();

        // 5. Serialize and save self.docmap to cache/docmap.bincode
        let docmap_file =
            File::create("cache/docmap.bincode").expect("Failed to create docmap file");
        let writer = BufWriter::new(docmap_file);

        bincode::serialize_into(writer, &self.docmap).expect("Failed to serialize docmap");

        println!(
            "Index build complete! Processed {} documents across {} blocks",
            self.docmap.len(),
            self.block_counter
        );
    }

    fn search(&self, query: &str) {
        //     // 1. Preprocess query with the exact same NLP pipeline:
        //     //    clean -> tokenize -> filter stop words -> stem.
        // clean
        let query_string: String = query.to_string();
        let cleaned_query: String = text_processor(&query_string);
        let tokenized_query: HashSet<&str> = tokenize::tokenizer(&cleaned_query);
        let query_no_stop: HashSet<&str> = remover_stop_words(tokenized_query);

        //     // 2. For each stemmed query token, call `self.get_documents(term)` to retrieve posting lists.

        //     // 3. Intersect (AND) or Union (OR) the posting lists to find matching doc IDs.

        //     // 4. Retrieve matching Movie structs using `self.docmap` (or `cache/docmap.bincode`).
    }
}

// Update the CLI entrypoint in `src/main.rs`:
// 1. Add a `Build` subcommand in the `Commands` enum alongside `Search`.
// 2. In `Commands::Build`:
//    - Initialize `InvertedIndex` and call `.build()` to generate blocks, merge them, and save docmap.
//    - Test verification: print the first document ID for token "merida", which should be
//      document 4651 ("Brave").
// 3. In `Commands::Search { query }`:
//    - Scrap the brute-force linear loop over all movies.
//    - Preprocess the query (clean -> tokenize -> filter stop words -> stem).
//    - For each stemmed query token, call `get_documents(term)` to retrieve its doc IDs.
//    - Intersect or union doc IDs, then fetch matching movies from `docmap`.
