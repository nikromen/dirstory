//! Deterministic history contracts, independent of generated operation sequences.

use super::*;

/// Build a known timeline, leaving its cursor at the final visit.
fn timeline(h: &mut History, paths: &[&str]) -> io::Result<()> {
    h.ensure(Path::new(paths[0]))?;
    for pair in paths.windows(2) {
        h.visit(Path::new(pair[0]), Path::new(pair[1]))?;
    }
    Ok(())
}

/// Assert the whole ordered timeline, current index and public selection revision.
fn assert_state(h: &mut History, paths: &[&str], cursor: usize, revision: u64) -> io::Result<()> {
    let current = h.select(true, 0)?.unwrap();
    assert_eq!(
        current.entry.path.as_os_str(),
        Path::new(paths[cursor]).as_os_str()
    );
    assert_eq!(current.revision, revision);
    let mut before = h.list(true, usize::MAX)?;
    before.reverse();
    let actual = before
        .into_iter()
        .map(|e| e.path.into_os_string())
        .chain(std::iter::once(current.entry.path.into_os_string()))
        .chain(
            h.list(false, usize::MAX)?
                .into_iter()
                .map(|e| e.path.into_os_string()),
        )
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        paths
            .iter()
            .map(std::ffi::OsString::from)
            .collect::<Vec<_>>()
    );
    Ok(())
}

fn move_cursor(h: &mut History, back: bool, n: usize) -> io::Result<Selection> {
    let selection = h.select(back, n)?.unwrap();
    h.commit(selection.revision, selection.entry.offset)?;
    Ok(selection)
}

#[test]
fn visit_from_middle_replaces_only_the_forward_branch() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    let mut h = History::open(dir.path(), true)?;
    timeline(&mut h, &["/a", "/b", "/c"])?;
    assert_state(&mut h, &["/a", "/b", "/c"], 2, 3)?;
    move_cursor(&mut h, true, 1)?;
    assert_state(&mut h, &["/a", "/b", "/c"], 1, 4)?;
    h.visit(Path::new("/b"), Path::new("/d"))?;
    assert_state(&mut h, &["/a", "/b", "/d"], 2, 5)?;
    assert!(h.select(false, 1)?.is_none());
    drop(h);
    assert_state(
        &mut History::open(dir.path(), true)?,
        &["/a", "/b", "/d"],
        2,
        5,
    )
}

#[test]
fn same_path_and_zero_steps_preserve_the_forward_branch_and_bytes() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    let mut h = History::open(dir.path(), true)?;
    timeline(&mut h, &["/a", "/b", "/c"])?;
    move_cursor(&mut h, true, 1)?;
    let file = dir.path().join("history.bin");
    let original = fs::read(&file)?;
    h.visit(Path::new("/b"), Path::new("/b"))?;
    for back in [true, false] {
        move_cursor(&mut h, back, 0)?;
    }
    h.ensure(Path::new("/unrelated"))?;
    assert_state(&mut h, &["/a", "/b", "/c"], 1, 4)?;
    assert_eq!(fs::read(file)?, original);
    Ok(())
}

#[test]
fn repeated_paths_are_distinct_visits_and_navigation_preserves_records() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    let mut h = History::open(dir.path(), true)?;
    timeline(&mut h, &["/a", "/b", "/a"])?;
    let last = h.select(true, 0)?.unwrap();
    let first = h.select(true, 2)?.unwrap();
    assert_eq!(first.entry.path, last.entry.path);
    assert_ne!(first.entry.offset, last.entry.offset);
    let file = dir.path().join("history.bin");
    let original = fs::read(&file)?;
    // Navigation to an earlier identical path still moves the visit cursor.
    move_cursor(&mut h, true, 2)?;
    assert_state(&mut h, &["/a", "/b", "/a"], 0, 4)?;
    move_cursor(&mut h, false, 1)?;
    assert_state(&mut h, &["/a", "/b", "/a"], 1, 5)?;
    move_cursor(&mut h, false, 1)?;
    assert_state(&mut h, &["/a", "/b", "/a"], 2, 6)?;
    move_cursor(&mut h, true, 1)?;
    assert_state(&mut h, &["/a", "/b", "/a"], 1, 7)?;
    // Record bytes are unchanged; only metadata may differ.
    assert_eq!(
        &fs::read(file)?[START as usize..],
        &original[START as usize..]
    );
    Ok(())
}

#[test]
fn boundary_selection_is_read_only_and_excess_steps_clamp() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    let mut h = History::open(dir.path(), true)?;
    timeline(&mut h, &["/a", "/b", "/c"])?;
    let file = dir.path().join("history.bin");
    let original = fs::read(&file)?;
    let s = h.select(true, usize::MAX)?.unwrap();
    assert_eq!(s.entry.path, Path::new("/a"));
    assert_eq!(fs::read(&file)?, original);
    h.commit(s.revision, s.entry.offset)?;
    assert_state(&mut h, &["/a", "/b", "/c"], 0, 4)?;
    let before = fs::read(&file)?;
    assert!(h.select(true, 1)?.is_none());
    assert_eq!(fs::read(&file)?, before);
    move_cursor(&mut h, false, usize::MAX)?;
    assert_state(&mut h, &["/a", "/b", "/c"], 2, 5)?;
    let before = fs::read(&file)?;
    assert!(h.select(false, 1)?.is_none());
    assert_eq!(fs::read(file)?, before);
    Ok(())
}

#[test]
fn stale_confirmation_preserves_the_newer_timeline() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    let mut h = History::open(dir.path(), true)?;
    timeline(&mut h, &["/a", "/b", "/c"])?;
    let selected = h.select(true, 1)?.unwrap();
    drop(h);
    let mut writer = History::open(dir.path(), true)?;
    writer.visit(Path::new("/c"), Path::new("/d"))?;
    drop(writer);
    let file = dir.path().join("history.bin");
    let before = fs::read(&file)?;
    let mut h = History::open(dir.path(), true)?;
    assert!(h.commit(selected.revision, selected.entry.offset).is_err());
    assert_state(&mut h, &["/a", "/b", "/c", "/d"], 3, 4)?;
    assert_eq!(fs::read(file)?, before);
    Ok(())
}

#[test]
fn mismatched_from_resets_even_when_destination_is_the_same() -> io::Result<()> {
    for new in ["/outside", "/destination"] {
        let dir = tempfile::tempdir()?;
        let mut h = History::open(dir.path(), true)?;
        timeline(&mut h, &["/a", "/b", "/c"])?;
        move_cursor(&mut h, true, 1)?;
        let old_token = h.select(false, 1)?.unwrap();
        h.visit(Path::new("/outside"), Path::new(new))?;
        if new == "/outside" {
            assert_state(&mut h, &["/outside"], 0, 5)?;
        } else {
            assert_state(&mut h, &["/outside", "/destination"], 1, 6)?;
        }
        let before = fs::read(dir.path().join("history.bin"))?;
        assert!(h
            .commit(old_token.revision, old_token.entry.offset)
            .is_err());
        assert_eq!(fs::read(dir.path().join("history.bin"))?, before);
    }
    Ok(())
}

/// Path component equality must not merge differently spelled shell PWD values.
#[test]
fn supplied_paths_are_compared_as_exact_unix_bytes() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    let mut h = History::open(dir.path(), true)?;
    timeline(&mut h, &["/a", "//a", "/a"])?;
    assert_state(&mut h, &["/a", "//a", "/a"], 2, 3)?;
    // A differently spelled from path must also trigger mismatch recovery.
    h.visit(Path::new("//a"), Path::new("//a"))?;
    assert_state(&mut h, &["//a"], 0, 4)
}
