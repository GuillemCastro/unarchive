use async_zip::error::ZipError;
use bytes::Bytes;
use std::fmt::Display;
use std::fmt::Formatter;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use thiserror::Error as ThisError;

pub struct Archive {
    kind: Kind,
    content: Content,
}

pub enum Kind {
    TAR,
    ZIP,
    GZIP,
}

pub enum Content {
    Path(PathBuf),
    Bytes(Bytes),
}

impl Archive {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, Error> {
        let kind = Self::infer_from_path(&path)?;
        Ok(Self {
            kind,
            content: Content::Path(path.as_ref().to_path_buf()),
        })
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let kind = Self::infer_from_bytes(&bytes)?;
        Ok(Self {
            kind,
            content: Content::Bytes(Bytes::copy_from_slice(bytes)),
        })
    }

    pub async fn unarchive(self, destination: impl AsRef<Path>) -> Result<(), Error> {
        if !destination.as_ref().exists() {
            tokio::fs::create_dir_all(destination.as_ref()).await?;
        }

        let bytes = match self.content {
            Content::Path(path) => tokio::fs::read(path).await?.into(),
            Content::Bytes(bytes) => bytes,
        };

        match self.kind {
            Kind::TAR => {
                tar::unarchive_tar(bytes, destination).await?;
            }
            Kind::ZIP => {
                zip::unarchive_zip(bytes, destination).await?;
            }
            Kind::GZIP => {
                gzip::unarchive_gzip(bytes, destination).await?;
            }
        }
        Ok(())
    }

    fn infer_from_path(path: impl AsRef<Path>) -> Result<Kind, Error> {
        infer::get_from_path(path)
            .map_err(|e| e.into())
            .and_then(|kind| match kind.map(|k| k.mime_type()) {
                Some("application/x-tar") => Ok(Kind::TAR),
                Some("application/zip") => Ok(Kind::ZIP),
                Some("application/gzip") => Ok(Kind::GZIP),
                Some(s) => Err(Error::InvalidFormat(Some(s.to_string()))),
                None => Err(Error::InvalidFormat(None)),
            })
    }

    fn infer_from_bytes(bytes: &[u8]) -> Result<Kind, Error> {
        infer::get(bytes)
            .map(|kind| match kind.mime_type() {
                "application/x-tar" => Ok(Kind::TAR),
                "application/zip" => Ok(Kind::ZIP),
                "application/gzip" => Ok(Kind::GZIP),
                s => Err(Error::InvalidFormat(Some(s.to_string()))),
            })
            .ok_or(Error::InvalidFormat(None))?
    }
}

#[derive(Debug, ThisError)]
pub enum IOError {
    StdIO(io::Error),
    ZipError(ZipError),
}

impl Display for IOError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StdIO(err) => write!(f, "Error reading the archive: {}", err),
            Self::ZipError(err) => write!(f, "Error reading ZIP archive: {}", err),
        }
    }
}

#[derive(ThisError, Debug)]
pub enum Error {
    #[error("error reading the archive")]
    IOError(IOError),
    #[error("the archive format `{0:?}` is not supported")]
    InvalidFormat(Option<String>),
    #[error("error: {0}")]
    Unknown(String),
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::IOError(IOError::StdIO(err))
    }
}

impl From<ZipError> for Error {
    fn from(err: ZipError) -> Self {
        Error::IOError(IOError::ZipError(err))
    }
}

mod zip;
mod tar;
mod gzip;