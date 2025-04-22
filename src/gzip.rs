use std::path::Path;

use async_compression::futures::bufread::GzipDecoder;
use async_tar::Archive;
use bytes::Bytes;
use futures::{io::Cursor, AsyncReadExt};
use tokio_util::compat::FuturesAsyncReadCompatExt;

use crate::Error;

pub(crate) async fn unarchive_gzip(bytes: Bytes, destination: impl AsRef<Path>) -> Result<(), Error> {
    let is_tar = is_targz(&bytes).await?;
    let decoder = GzipDecoder::new(Cursor::new(bytes));
    
    if is_tar {
        let ar = Archive::new(decoder);

        ar.unpack(destination.as_ref()).await?;
    } else {
        let mut file = tokio::fs::File::create(destination.as_ref()).await?;
        tokio::io::copy(&mut decoder.compat(), &mut file).await?;
    }

    Ok(())
}

async fn is_targz(bytes: &[u8]) -> Result<bool, Error> {
    let decoder = GzipDecoder::new(Cursor::new(bytes));

    let mut out = vec![0; 8 * 1024];
    decoder.take(8 * 1024).read(&mut out).await?;
    let kind = infer::get(&out);

    return match kind.map(|k| k.mime_type()) {
        Some("application/x-tar") => Ok(true),
        _ => Ok(false),
    };
}