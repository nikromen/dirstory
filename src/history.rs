//! Directory history stored as an offset-based sequence in a single file.
//!
//! A 64-byte header stores the format version, dirty flag, cursor, data end,
//! and revision. Each record contains `[u64 length][Unix path bytes][u64 length]`,
//! with little-endian integers. Both lengths permit traversal without an index.
//!
//! Navigation reads neighboring records and updates only the header. New visits
//! truncate the forward branch before appending.
//!
//! A separate advisory lock is held for the lifetime of [`History`]. Dirty files
//! require an explicit reset. Writes are not durable: there is no journal or fsync.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::{ffi::OsStrExt, ffi::OsStringExt, fs::OpenOptionsExt, fs::PermissionsExt};
use std::path::{Path, PathBuf};

/// Byte offset of the first record, immediately after the fixed-size header.
const START: u64 = 64;

/// File signature identifying the dirstory binary format.
const MAGIC: &[u8; 8] = b"DIRSTORY";

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Invalid or interrupted history; use dirstory internal reset PATH",
    )
}

#[derive(Clone, Copy, Debug)]
struct Header {
    /// Start offset of the current visit.
    current: u64,

    /// First byte beyond the valid records.
    end: u64,

    /// Generation used to detect changes between selection and confirmation.
    revision: u64,
}

#[derive(Debug)]
pub struct Entry {
    /// Start of the record, used as its identity within a revision.
    pub offset: u64,

    /// Directory path stored as raw Unix bytes.
    pub path: PathBuf,

    /// Offset immediately after the trailing length field.
    pub end: u64,
}

/// Read-only navigation result to confirm after a successful shell cd.
#[derive(Debug)]
pub struct Selection {
    /// Target visit; selecting it does not change the cursor.
    pub entry: Entry,

    pub revision: u64,
}

/// Open history file together with a lock held until this value is dropped.
pub struct History {
    file: File,

    // Keep the lock descriptor alive for the complete operation.
    _lock: File,
}

impl History {
    /// Open or create the files and acquire the session lock.
    ///
    /// Use an exclusive lock for every mutating method, or a shared lock for
    /// selection and listing. This does not initialize or validate the contents.
    /// Directory permissions are 0700 and file permissions are 0600.
    ///
    /// Returns filesystem or lock errors; lock acquisition may block.
    pub fn open(dir: &Path, exclusive: bool) -> io::Result<Self> {
        fs::DirBuilder::new().recursive(true).create(dir)?;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;

        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(dir.join("history.lock"))?;

        if exclusive {
            lock.lock()?;
        } else {
            lock.lock_shared()?;
        }

        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(dir.join("history.bin"))?;

        lock.set_permissions(fs::Permissions::from_mode(0o600))?;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;

        Ok(Self { file, _lock: lock })
    }

    /// Read exactly the requested bytes, treating premature EOF as corruption.
    fn read_at(&mut self, pos: u64, buf: &mut [u8]) -> io::Result<()> {
        self.file.seek(SeekFrom::Start(pos))?;
        self.file.read_exact(buf).map_err(|e| {
            if e.kind() == io::ErrorKind::UnexpectedEof {
                invalid()
            } else {
                e
            }
        })
    }

    /// Decode one little-endian u64 at the given file offset.
    fn number(&mut self, pos: u64) -> io::Result<u64> {
        let mut b = [0; 8];
        self.read_at(pos, &mut b)?;
        Ok(u64::from_le_bytes(b))
    }

    /// Validate the header and current record without scanning older visits.
    fn header(&mut self) -> io::Result<Header> {
        let mut b = [0; 64];
        self.read_at(0, &mut b)?;
        if &b[..8] != MAGIC
            || u64::from_le_bytes(b[8..16].try_into().unwrap()) != 1
            || b[16..24] != [0; 8]
            || b[48..] != [0; 16]
        {
            return Err(invalid());
        }
        let h = Header {
            current: u64::from_le_bytes(b[24..32].try_into().unwrap()),
            end: u64::from_le_bytes(b[32..40].try_into().unwrap()),
            revision: u64::from_le_bytes(b[40..48].try_into().unwrap()),
        };
        if h.current < START || h.current >= h.end || h.end != self.file.metadata()?.len() {
            return Err(invalid());
        }

        self.entry(h.current, h.end)?;
        Ok(h)
    }

    /// Write metadata with a dirty marker cleared only after completion.
    ///
    /// When `dirty` is true, leave the marker set for subsequent record writes.
    fn write_header(&mut self, h: Header, dirty: bool) -> io::Result<()> {
        let mut b = [0; 64];
        b[..8].copy_from_slice(MAGIC);
        for (i, n) in [1, 1, h.current, h.end, h.revision].into_iter().enumerate() {
            b[8 + i * 8..16 + i * 8].copy_from_slice(&n.to_le_bytes());
        }

        // Mark dirty before changing any metadata, and clear it only after
        // the complete header has been written. This is process-interruption
        // detection, not a crash recovery (there is no fsync).
        self.file.seek(SeekFrom::Start(16))?;
        self.file.write_all(&[1])?;
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(&b)?;
        if !dirty {
            self.file.seek(SeekFrom::Start(16))?;
            self.file.write_all(&[0])?;
        }
        Ok(())
    }

    /// Validate record boundaries and paired lengths before allocating its path.
    fn entry(&mut self, offset: u64, limit: u64) -> io::Result<Entry> {
        if offset < START || offset.checked_add(16).is_none_or(|end| end > limit) {
            return Err(invalid());
        }
        let len = self.number(offset)?;
        let end = offset
            .checked_add(16)
            .and_then(|n| n.checked_add(len))
            .filter(|n| *n <= limit)
            .ok_or_else(invalid)?;
        if self.number(end - 8)? != len {
            return Err(invalid());
        }

        let size = usize::try_from(len).map_err(|_| invalid())?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(size).map_err(|_| invalid())?;
        bytes.resize(size, 0);
        self.read_at(offset + 8, &mut bytes)?;
        if bytes.is_empty() || bytes.contains(&0) {
            return Err(invalid());
        }

        Ok(Entry {
            offset,
            path: std::ffi::OsString::from_vec(bytes).into(),
            end,
        })
    }

    fn append(&mut self, offset: u64, path: &Path) -> io::Result<u64> {
        let b = path.as_os_str().as_bytes();
        if b.is_empty() || b.contains(&0) {
            return Err(invalid());
        }
        let len = u64::try_from(b.len()).map_err(|_| invalid())?;
        let end = offset
            .checked_add(16)
            .and_then(|n| n.checked_add(len))
            .ok_or_else(invalid)?;
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.write_all(&len.to_le_bytes())?;
        self.file.write_all(b)?;
        self.file.write_all(&len.to_le_bytes())?;
        Ok(end)
    }

    pub fn reset(&mut self, path: &Path) -> io::Result<()> {
        let revision = if self.file.metadata()?.len() >= START {
            self.number(40)?.checked_add(1).unwrap_or(1)
        } else {
            1
        };
        let mut h = Header {
            current: START,
            end: START,
            revision,
        };
        self.write_header(h, true)?;
        self.file.set_len(START)?;
        h.end = self.append(START, path)?;
        self.write_header(h, false)
    }

    pub fn ensure(&mut self, path: &Path) -> io::Result<()> {
        if self.file.metadata()?.len() == 0 {
            self.reset(path)
        } else {
            self.header().map(|_| ())
        }
    }

    pub fn visit(&mut self, old: &Path, new: &Path) -> io::Result<()> {
        self.ensure(old)?;
        let mut h = self.header()?;
        if self.entry(h.current, h.end)?.path != old {
            self.reset(old)?;
            h = self.header()?;
        }
        if old == new {
            return Ok(());
        }
        let offset = self.entry(h.current, h.end)?.end;
        h.revision = h.revision.checked_add(1).ok_or_else(invalid)?;
        self.write_header(h, true)?;
        self.file.set_len(offset)?;
        h.current = offset;
        h.end = self.append(offset, new)?;
        self.write_header(h, false)
    }

    /// Read the adjacent visit, or return None at the requested boundary.
    fn neighbor(&mut self, e: &Entry, h: Header, back: bool) -> io::Result<Option<Entry>> {
        if back {
            if e.offset == START {
                return Ok(None);
            }
            let length = self.number(e.offset.checked_sub(8).ok_or_else(invalid)?)?;
            let pos = e
                .offset
                .checked_sub(16)
                .and_then(|n| n.checked_sub(length))
                .ok_or_else(invalid)?;
            let prev = self.entry(pos, h.end)?;
            if prev.end != e.offset {
                return Err(invalid());
            }
            Ok(Some(prev))
        } else if e.end == h.end {
            Ok(None)
        } else {
            self.entry(e.end, h.end).map(Some)
        }
    }

    /// Choose a target without updating history; `back` selects earlier visits.
    pub fn select(&mut self, back: bool, n: usize) -> io::Result<Option<Selection>> {
        let h = self.header()?;
        let mut e = self.entry(h.current, h.end)?;
        if n == 0 {
            return Ok(Some(Selection {
                entry: e,
                revision: h.revision,
            }));
        }

        let mut moved = false;
        for _ in 0..n {
            match self.neighbor(&e, h, back)? {
                Some(next) => {
                    e = next;
                    moved = true;
                }
                None => break,
            }
        }

        Ok(moved.then_some(Selection {
            entry: e,
            revision: h.revision,
        }))
    }

    /// Confirm a selected offset after the shell has successfully performed cd.
    pub fn commit(&mut self, revision: u64, offset: u64) -> io::Result<()> {
        let mut h = self.header()?;
        if h.revision != revision {
            return Err(io::Error::other(
                "History changed since selection; directory has already changed",
            ));
        }
        self.entry(offset, h.end)?;
        if h.current == offset {
            return Ok(());
        }

        h.current = offset;
        h.revision = h.revision.checked_add(1).ok_or_else(invalid)?;
        self.write_header(h, true)?;
        self.write_header(h, false)
    }

    pub fn list(&mut self, back: bool, n: usize) -> io::Result<Vec<Entry>> {
        let h = self.header()?;
        let mut e = self.entry(h.current, h.end)?;
        let mut entries = Vec::new();
        for _ in 0..n {
            match self.neighbor(&e, h, back)? {
                Some(next) => {
                    entries.push(next);
                    e = self.entry(entries.last().unwrap().offset, h.end)?;
                }
                None => break,
            }
        }
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_path_bytes_roundtrip() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let mut h = History::open(dir.path(), true)?;
        let path = PathBuf::from(std::ffi::OsString::from_vec(b"/non-utf8-\xff\n".to_vec()));
        h.reset(&path)?;
        assert_eq!(h.select(true, 0)?.unwrap().entry.path, path);
        Ok(())
    }

    #[test]
    fn local_navigation_does_not_scan_earlier_entries() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let mut h = History::open(dir.path(), true)?;
        h.reset(Path::new("/a"))?;
        h.visit(Path::new("/a"), Path::new("/b"))?;
        h.visit(Path::new("/b"), Path::new("/c"))?;
        // A damaged old entry does not affect the immediate C -> B step.
        h.file.seek(SeekFrom::Start(START))?;
        h.file.write_all(&u64::MAX.to_le_bytes())?;
        assert_eq!(h.select(true, 1)?.unwrap().entry.path, Path::new("/b"));
        assert!(h.select(true, 2).is_err());
        Ok(())
    }

    #[test]
    fn navigation_and_branching() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let mut h = History::open(dir.path(), true)?;
        h.ensure(Path::new("/a"))?;
        h.visit(Path::new("/a"), Path::new("/bb"))?;
        h.visit(Path::new("/bb"), Path::new("/ccc"))?;
        let s = h.select(true, 1)?.unwrap();
        assert_eq!(s.entry.path, Path::new("/bb"));
        h.commit(s.revision, s.entry.offset)?;
        assert_eq!(h.list(false, 10)?[0].path, Path::new("/ccc"));
        h.visit(Path::new("/bb"), Path::new("/dddd"))?;
        assert!(h.select(false, 1)?.is_none());
        assert!(h.commit(s.revision, s.entry.offset).is_err());
        let s = h.select(true, 99)?.unwrap();
        assert_eq!(s.entry.path, Path::new("/a"));
        h.commit(s.revision, s.entry.offset)?;
        assert_eq!(h.list(false, 10)?.len(), 2);
        drop(h);
        let mut h = History::open(dir.path(), true)?;
        assert_eq!(h.select(false, 99)?.unwrap().entry.path, Path::new("/dddd"));
        Ok(())
    }

    #[test]
    fn corruption_and_reset() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let mut h = History::open(dir.path(), true)?;
        h.reset(Path::new("/a"))?;
        let header = h.header()?;
        h.write_header(header, true)?;
        assert!(h.select(true, 1).is_err());
        h.reset(Path::new("/b"))?;
        h.file.seek(SeekFrom::Start(START))?;
        h.file.write_all(&u64::MAX.to_le_bytes())?;
        assert!(h.list(true, 1).is_err());
        Ok(())
    }

    #[test]
    fn repeated_paths_and_zero() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let mut h = History::open(dir.path(), true)?;
        h.reset(Path::new("/a"))?;
        h.visit(Path::new("/a"), Path::new("/b"))?;
        h.visit(Path::new("/b"), Path::new("/a"))?;
        assert_eq!(h.list(true, 20)?.len(), 2);
        let s = h.select(true, 0)?.unwrap();
        let rev = s.revision;
        h.commit(rev, s.entry.offset)?;
        assert_eq!(h.select(true, 0)?.unwrap().revision, rev);
        h.visit(Path::new("/a"), Path::new("/a"))?;
        assert_eq!(h.list(true, 20)?.len(), 2);
        Ok(())
    }
}
