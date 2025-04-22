# unarchive

Simple crate to unarchive some common archive file formats

```rust
use std::path::PathBuf;

use unarchive::{Archive, Error};

#[tokio::main]
pub async fn main() -> Result<(), Error> {
    Archive::from_path("example.tar.gz")?.unarchive("unarchived").await
}

```

## License

This project is licensed under the [Apache 2.0 license](LICENSE)
