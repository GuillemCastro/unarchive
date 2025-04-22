use std::path::Path;

use async_tar::Archive;
use bytes::Bytes;
use futures::io::Cursor;

use crate::Error;

pub(crate) async fn unarchive_tar(bytes: Bytes, destination: impl AsRef<Path>) -> Result<(), Error> {
    let ar = Archive::new(Cursor::new(bytes));

    ar.unpack(destination.as_ref()).await?;

    Ok(())
}