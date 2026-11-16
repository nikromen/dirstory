//! Deterministic complexity checks; no wall-clock performance assertions.
use super::*;
use std::cell::RefCell;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Traffic {
    reads: usize,
    read_bytes: usize,
    writes: usize,
    write_bytes: usize,
    lengths: usize,
    truncations: usize,
}

#[derive(Default)]
struct CountingStorage {
    bytes: Vec<u8>,
    traffic: RefCell<Traffic>,
}

impl Storage for CountingStorage {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        let mut traffic = self.traffic.borrow_mut();
        traffic.reads += 1;
        traffic.read_bytes += buf.len();

        let start = offset as usize;
        let source = self
            .bytes
            .get(start..start + buf.len())
            .ok_or_else(|| io::Error::from(io::ErrorKind::UnexpectedEof))?;
        buf.copy_from_slice(source);
        Ok(())
    }

    fn write_at(&mut self, offset: u64, buf: &[u8]) -> io::Result<()> {
        let mut traffic = self.traffic.borrow_mut();
        traffic.writes += 1;
        traffic.write_bytes += buf.len();

        let start = offset as usize;
        self.bytes
            .resize(self.bytes.len().max(start + buf.len()), 0);
        self.bytes[start..start + buf.len()].copy_from_slice(buf);
        Ok(())
    }

    fn length(&self) -> io::Result<u64> {
        self.traffic.borrow_mut().lengths += 1;
        Ok(self.bytes.len() as u64)
    }

    fn set_len(&mut self, len: u64) -> io::Result<()> {
        self.traffic.borrow_mut().truncations += 1;
        self.bytes.resize(len as usize, 0);
        Ok(())
    }
}

#[test]
fn navigation_and_fixed_lists_have_constant_io_across_history_sizes() -> io::Result<()> {
    let mut baseline = None;

    for count in [100, 10_000, 100_000] {
        let mut history = History {
            file: CountingStorage::default(),
        };
        let paths = [Path::new("/a"), Path::new("/b")];
        history.reset(paths[0])?;
        for index in 1..count {
            history.visit(paths[(index - 1) % 2], paths[index % 2])?;
        }

        // Position in the middle outside the measured operations. All paths
        // have equal byte lengths, while the older prefix grows with count.
        let middle = history.select(true, count / 2)?.unwrap();
        history.commit(middle.revision, middle.entry.offset)?;
        let mut measurements = Vec::new();

        for back in [true, false] {
            history.file.traffic.replace(Traffic::default());
            let target = history.select(back, 1)?.unwrap();
            history.commit(target.revision, target.entry.offset)?;
            let traffic = history.file.traffic.borrow().clone();
            assert!(traffic.read_bytes > 0 && traffic.write_bytes > 0);
            measurements.push(traffic);
        }

        for back in [true, false] {
            history.file.traffic.replace(Traffic::default());
            assert_eq!(history.list(back, 10)?.len(), 10);
            let traffic = history.file.traffic.borrow().clone();
            assert!(traffic.read_bytes > 0);
            assert_eq!(traffic.write_bytes, 0);
            measurements.push(traffic);
        }

        if let Some(expected) = &baseline {
            assert_eq!(&measurements, expected, "I/O grew at {count} visits");
        } else {
            baseline = Some(measurements);
        }
    }

    Ok(())
}
