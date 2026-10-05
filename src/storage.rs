//! File operations and session locking, independent of the history format.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::unix::fs::{FileExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

/// Operations needed by the history encoder and navigator.
///
/// Implementations must complete each requested read or write, or return an
/// error. Writes may have modified part of the file before returning an error.
pub trait Storage {
    /// Read exactly `buf.len()` bytes at the given offset.
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<()>;

    /// Write all bytes at the given offset.
    fn write_at(&mut self, offset: u64, buf: &[u8]) -> io::Result<()>;

    /// Return the current file length.
    fn length(&self) -> io::Result<u64>;

    /// Truncate or extend the file to the requested length.
    fn set_len(&mut self, len: u64) -> io::Result<()>;
}

/// Real file storage retaining its session lock until dropped.
pub struct FileStorage {
    file: File,

    // The separate descriptor keeps the lock stable across history truncation.
    _lock: File,
}

impl FileStorage {
    /// Create private session files and acquire the requested advisory lock.
    pub(super) fn open(dir: &Path, exclusive: bool) -> io::Result<Self> {
        fs::DirBuilder::new().recursive(true).create(dir)?;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;

        let lock = Self::open_file(&dir.join("history.lock"))?;
        if exclusive {
            lock.lock()?;
        } else {
            lock.lock_shared()?;
        }

        let file = Self::open_file(&dir.join("history.bin"))?;
        lock.set_permissions(fs::Permissions::from_mode(0o600))?;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
        Ok(Self { file, _lock: lock })
    }

    fn open_file(path: &Path) -> io::Result<File> {
        OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(path)
    }
}

impl Storage for FileStorage {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        self.file.read_exact_at(buf, offset)
    }

    fn write_at(&mut self, offset: u64, buf: &[u8]) -> io::Result<()> {
        self.file.write_all_at(buf, offset)
    }

    fn length(&self) -> io::Result<u64> {
        Ok(self.file.metadata()?.len())
    }

    fn set_len(&mut self, len: u64) -> io::Result<()> {
        self.file.set_len(len)
    }
}
