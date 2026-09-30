use serde::Deserialize;
use std::error::Error;
use std::fmt;
use std::fmt::Display;
use std::fs::File;
use std::io::BufReader;

#[derive(Deserialize)]
pub struct MovieData {
    pub movies: Vec<Movie>,
}

#[derive(Deserialize)]
pub struct Movie {
    // How is a movie represented in json file
    pub id: u32,
    pub title: String,
    pub description: String,
}
// Implementing `Display` trait for Movie struct
impl Display for Movie {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "id: {}\n\ntitle: {}\n\ndescription: {}",
            self.id, self.title, self.description
        )
    }
}

fn movie_giver(coll_mov: MovieData) -> Vec<Movie> {
    coll_mov.movies
}

pub fn json_loader() -> Result<Vec<Movie>, Box<dyn Error>> {
    // Open file
    let file = File::open("data/movies.json")?;

    // Read onto a BufReader
    let reader = BufReader::new(file); // This is required since if it were
    // not present then the program
    // would ask for the json byte by
    // byte which would be awfully slow.
    // BufReader asks for text in chunks
    // and therefore it is much faster
    // to use it.

    // Parse json array into a vector or Movies
    let movies_data: MovieData = serde_json::from_reader(reader)?;
    // we'll have to extract the Movies out of MovieData

    let movies: Vec<Movie> = movie_giver(movies_data);
    Ok(movies)
}

/*pub fn test_data_working() {
    let some_movies = json_loader().unwrap();
    for movie in some_movies {
        println!("{:?}", movie);
    }
}*/

/* #[cfg(test)]
mod tests {
    use super::*;

    #[test]
} */
