pub mod data_loader;

use crate::data_loader::{Movie, json_loader};
use clap::{Parser, Subcommand};

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
    /// Search movies using keyword
    // This statement is printed when help is called wow!
    Search {
        // This means, now we have '-- search' sub-command
        #[arg(help = "Search query")]
        query: String, // Field of the search struct
    },
}

fn main() {
    let cli = Cli::parse(); // This initializes the CLI
    // This is possible because of `Cli` implementing `Parser` trait.

    match &cli.command {
        Commands::Search { query } => {
            println!("Searching for: {}", query);

            // Clean the query of punctuation
            let cleaned_query: String = query
                .replace(|c: char| c.is_ascii_punctuation(), "")
                .to_lowercase();
            // Bring in json data
            let movies: Vec<Movie> = json_loader().expect("Failed to load movies");
            // We can search for the query in the `Movie` struct.
            // Iterate through the movies to find the query
            // print all the names one by one
            let queried: Vec<&Movie> = movies
                .iter()
                .filter(|movie| {
                    let cleaned_title = movie
                        .title
                        .replace(|c: char| c.is_ascii_punctuation(), "")
                        .to_lowercase();
                    cleaned_title.contains(&cleaned_query) // contains only accepts string slice as
                    // parameter, but it compares it to a
                    // String and not &str.
                    // cleaned_title needs to be String
                    // cleaned_query needs to be &str here
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
