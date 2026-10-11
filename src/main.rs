pub mod data_loader;
pub mod inverted_index;
pub mod stemming;
pub mod stop_word_remover;
pub mod text_processing;
pub mod tokenize;

use crate::data_loader::{Movie, json_loader};
use crate::inverted_index::InvertedIndex;
use crate::stop_word_remover::get_stop_words;
use crate::tokenize::tokenizer;
use clap::{Parser, Subcommand};
use std::collections::HashSet;
use stemming::stemmer;
use text_processing::text_processor;

#[derive(Parser)]
// Adds the corresponding struct to implement the `Parser` trait
// and have all the respective impl methods
// #[command(name = "keyword_search_cli")] // This is the name of the command
#[command(about = "Keyword Search CLI - Available commands")] // This is printed on top of the help
// menu
struct Cli {
    #[command(subcommand)]
    command: Commands, // 'Commands' is a enum, which means one sub-command at a time..
}

#[derive(Subcommand)] // With this each variant becomes valid sub-command
enum Commands {
    /// Build and save the inverted_index nad docmap
    Build,
    /// Search movies using keyword
    // This statement is printed when help is called wow!
    Search {
        // The name of the struct means, now we have '-- search' sub-command
        // But the command will be the name of the struct but in lowercase
        #[arg(help = "Search query")]
        query: String, // Field of the search struct
    },
}

fn main() {
    let cli = Cli::parse(); // This initializes the CLI
    // This is possible because of `Cli` implementing `Parser` trait.

    match &cli.command {
        Commands::Build => {
            println!("Building inverted index...");
            let mut index: InvertedIndex = inverted_index::InvertedIndex {
                index: std::collections::HashMap::new(),
                docmap: std::collections::HashMap::new(),
                block_size_limit: 500, // eg. flush to disk every 500 documents
                block_counter: 0,
                docs_in_current_batch: 0,
            };
            index.build();
        }
        Commands::Search { query } => {
            println!("Searching for: {}\n", query);

            // STOP WORDS
            let stop_words_text: String = get_stop_words();

            // TODO: structure this code such that it can be called as a single
            // function and we get a `HashSet` with no stop words in it.
            let stop_words: HashSet<&str> =
                stop_words_text.lines().map(|line| line.trim()).collect();
            // we don't need to tokenize it since it's already words in the file.

            /* println!("stop_words: {:?}", stop_words); */

            // Clean the query of punctuation
            let cleaned_query: String = text_processor(query);

            let mut cleaned_query_tokenized: HashSet<&str> = tokenizer(&cleaned_query);

            cleaned_query_tokenized.retain(|item| !stop_words.contains(item));

            let stemmed_query_cow = stemmer(&cleaned_query_tokenized);
            let stemmed_query: HashSet<&str> =
                stemmed_query_cow.iter().map(|cow| cow.as_ref()).collect();

            // Bring in json data
            let movies: Vec<Movie> = json_loader().expect("Failed to load movies");
            // We can search for the query in the `Movie` struct.
            // Iterate through the movies to find the query
            // print all the names one by one
            let queried: Vec<&Movie> = movies
                .iter()
                .filter(|movie| {
                    // NOTE: We are writing the condition
                    // for filter, so a boolean is expected such that
                    // if true it could be filtered and if not then
                    // not filtered.
                    /* let cleaned_title = movie
                    .title
                    .replace(|c: char| c.is_ascii_punctuation(), "")
                    .to_lowercase(); */
                    let cleaned_title = text_processor(&movie.title);

                    let mut cleaned_title_tokenized: HashSet<&str> = tokenizer(&cleaned_title);

                    cleaned_title_tokenized.retain(|item| !stop_words.contains(item));

                    let stemmed_title_cow = stemmer(&cleaned_title_tokenized);
                    let stemmed_title: HashSet<&str> =
                        stemmed_title_cow.iter().map(|cow| cow.as_ref()).collect();

                    /*println!(
                        "cleaned_title_tokenized: {:?}\ncleaned_query_tokenized: {:?}",
                        cleaned_title_tokenized, cleaned_query_tokenized
                    );*/
                    // used for debugging

                    /* cleaned_title.contains(&cleaned_query) */
                    // contains only accepts string slice as
                    // parameter, but it compares it to a
                    // String and not &str.
                    // cleaned_title needs to be String
                    // cleaned_query needs to be &str here
                    //
                    //
                    // see if tokenized query vec![] contains contains
                    // any &str of the tokenized title vec![]
                    // compare both the vectors and if in tokenized
                    // query contains any &str of tokenized title
                    // get the movie
                    // Or we might use a HashSet
                    /* !cleaned_title_tokenized.is_disjoint(&cleaned_query_tokenized)*/
                    //
                    /* cleaned_query_tokenized
                    .iter()
                    .any(|query_word| cleaned_title.contains(query_word)) */
                    // Based on comparing token this is the code
                    stemmed_title.iter().any(|title_token| {
                        stemmed_query
                            .iter()
                            .any(|query_token| title_token.contains(query_token))
                    })
                })
                .collect();

            println!("Found {} movie(s) with {} in title", queried.len(), query);

            for movie_title in queried {
                println!("{}", movie_title);
                print!("\n\n\n\n");
            }
        }
    }
}
