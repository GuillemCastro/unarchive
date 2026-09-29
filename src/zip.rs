use std::path::{Component, Path, PathBuf};

use async_zip::base::read::seek::ZipFileReader;
use bytes::Bytes;
use futures::io::Cursor;
use tokio_util::compat::FuturesAsyncReadCompatExt;

use crate::Error;

/// Join an archive-supplied entry `name` onto `base`, confining the result to
/// `base`. Returns `None` for any entry that would escape it.
///
/// ZIP entry names come straight from the archive so joining them onto the destination
/// with `Path::join` is a path-traversal / arbitrary-file-write vulnerability
/// (zip-slip, CWE-22): a `..` component climbs out of the destination and an
/// absolute path (or Windows drive prefix) discards `base` entirely.
///
/// Confinement is purely lexical (it does not resolve symlinks); that is
/// sufficient here because extraction only ever creates directories and regular
/// files, so the archive cannot plant a symlink for a later entry to follow.
/// a well formed zip never contains a `..` so it rejected outright.
///
/// ZIP names are `/`-separated per the spec; `\` is normalized to `/` as well
/// so a `..\..\evil` name from a non-conformant producer cannot slip past the
/// component check on platforms where `\` is not a separator.
fn safe_join(base: &Path, name: &str) -> Option<PathBuf> {
    let normalized = name.replace('\\', "/");
    let mut out = base.to_path_buf();
    let mut segments = 0usize;
    for component in Path::new(&normalized).components() {
        match component {
            Component::Normal(part) => {
                out.push(part);
                segments += 1;
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    // Reject entries that resolve to no real path segment (e.g. "", ".").
    if segments == 0 {
        return None;
    }
    Some(out)
}

pub(crate) async fn unarchive_zip(
    bytes: Bytes,
    destination: impl AsRef<Path>,
) -> Result<(), Error> {
    let mut reader = ZipFileReader::new(Cursor::new(bytes)).await?;

    for index in 0..reader.file().entries().len() {
        let entry = reader.file().entries()[index].clone();
        let filename = entry.filename().as_str()?;

        // Confine the entry to the destination directory before touching the
        // filesystem, rejecting any name that would escape it.
        let path = safe_join(destination.as_ref(), filename)
            .ok_or_else(|| Error::UnsafeEntry(filename.to_string()))?;

        if entry.dir()? {
            tokio::fs::create_dir_all(&path).await?;
            continue;
        }

        // Ensure the parent chain exists.
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let entry_reader = reader.reader_without_entry(index).await?;

        let mut file = tokio::fs::File::create(&path).await?;
        tokio::io::copy(&mut entry_reader.compat(), &mut file).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::safe_join;
    use std::path::{Path, PathBuf};

    fn base() -> PathBuf {
        PathBuf::from("/dest")
    }

    #[test]
    fn rejects_parent_traversal() {
        assert_eq!(safe_join(&base(), "../ESCAPED.txt"), None);
        assert_eq!(safe_join(&base(), "../../ESCAPED.txt"), None);
        assert_eq!(safe_join(&base(), "a/../../ESCAPED.txt"), None);
        // strict: any `..` is rejected, even one that would net-resolve inside base.
        assert_eq!(safe_join(&base(), "foo/../bar.txt"), None);
    }

    #[test]
    fn rejects_absolute_and_prefix() {
        assert_eq!(safe_join(&base(), "/etc/passwd"), None);
        assert_eq!(safe_join(&base(), "/tmp/pwned.txt"), None);
    }

    #[test]
    fn rejects_backslash_traversal() {
        // `\` is normalized to a separator so it can't hide a `..` on unix.
        assert_eq!(safe_join(&base(), "..\\..\\ESCAPED.txt"), None);
    }

    #[test]
    fn rejects_empty_or_dot_only() {
        assert_eq!(safe_join(&base(), ""), None);
        assert_eq!(safe_join(&base(), "."), None);
    }

    #[test]
    fn allows_plain_and_nested() {
        assert_eq!(
            safe_join(&base(), "file.txt"),
            Some(Path::new("/dest/file.txt").to_path_buf())
        );
        assert_eq!(
            safe_join(&base(), "data/foo/bar.txt"),
            Some(Path::new("/dest/data/foo/bar.txt").to_path_buf())
        );
        assert_eq!(
            safe_join(&base(), "./data/bar.txt"),
            Some(Path::new("/dest/data/bar.txt").to_path_buf())
        );
    }

    #[test]
    fn allows_directory_entry() {
        // ZIP directory entries end with `/`; the trailing separator yields no
        // extra component, so they confine like any other nested path.
        assert_eq!(
            safe_join(&base(), "sub/dir/"),
            Some(Path::new("/dest/sub/dir").to_path_buf())
        );
    }

    #[test]
    fn accepted_paths_stay_under_base() {
        let b = base();
        for name in [
            "f.txt",
            "a/b/c.dat",
            "../x",
            "..\\x",
            "/abs",
            "a/../../x",
            "",
        ] {
            if let Some(joined) = safe_join(&b, name) {
                assert!(
                    joined.starts_with(&b),
                    "entry {:?} escaped: {:?}",
                    name,
                    joined
                );
            }
        }
    }
}
