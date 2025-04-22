use std::path::Path;

use async_zip::base::read::seek::ZipFileReader;
use bytes::Bytes;
use futures::io::Cursor;
use tokio_util::compat::FuturesAsyncReadCompatExt;

use crate::Error;

pub(crate) async fn unarchive_zip(bytes: Bytes, destination: impl AsRef<Path>) -> Result<(), Error> {
    let mut reader = ZipFileReader::new(Cursor::new(bytes)).await?;

    for index in 0..reader.file().entries().len() {
        let entry = reader.file().entries()[index].clone();
        let filename = entry.filename().as_str()?;

        let path = destination.as_ref().join(filename);

        match path.parent() {
            Some(parent) => {
                if !parent.exists() {
                    tokio::fs::create_dir_all(parent).await?;
                }
            }
            None => {}
        }

        if entry.dir()? {
            tokio::fs::create_dir_all(destination.as_ref().join(filename)).await?;
            continue;
        }

        let entry_reader = reader.reader_without_entry(index).await?;

        let mut file = tokio::fs::File::create(destination.as_ref().join(filename)).await?;
        tokio::io::copy(&mut entry_reader.compat(), &mut file).await?;
    }
    Ok(())
}