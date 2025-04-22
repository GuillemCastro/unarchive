use std::path::PathBuf;

use unarchive::{Archive, Error};

#[tokio::main]
pub async fn main() -> Result<(), Error> {
    let path = PathBuf::from("examples/example.tar.gz");
    println!("Unarchiving tar.gz file: {:?}", path);

    Archive::from_path(path)?.unarchive("unarchived-targz").await
}
