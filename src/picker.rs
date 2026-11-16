//! Snapshot selection, drawing, and terminal ownership are kept independent.
mod search;

use crate::history::{Selection, Snapshot};
use crossterm::{
    cursor::Show,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use search::Search;
use std::{
    fs::{File, OpenOptions},
    io, panic,
    sync::Arc,
    time::Duration,
};

struct Picker {
    snapshot: Snapshot,
    texts: Arc<Vec<String>>,
    search: Search,
    query: String,
    matches: Vec<usize>,
    selected: Option<usize>,
    pending: bool,
    rows: usize,
    visible: bool,
    list: ListState,
}

impl Picker {
    fn new(snapshot: Snapshot) -> io::Result<Self> {
        let selected = snapshot
            .entries
            .iter()
            .position(|e| e.offset == snapshot.current)
            .expect("validated snapshot has a current visit");

        let texts = Arc::new(
            snapshot
                .entries
                .iter()
                .map(|entry| display_path(&entry.path))
                .collect(),
        );
        let search = Search::new(Arc::clone(&texts))?;
        let matches = (0..snapshot.entries.len()).collect();

        Ok(Self {
            snapshot,
            texts,
            search,
            query: String::new(),
            matches,
            selected: Some(selected),
            pending: false,
            rows: 1,
            visible: false,
            list: ListState::default(),
        })
    }

    fn resize(&mut self, width: u16, height: u16) {
        self.visible = width >= 20 && height >= 5;
        self.rows = usize::from(height.saturating_sub(4)).max(1);
    }

    fn navigate(&mut self, key: KeyCode) {
        if self.pending || self.matches.is_empty() {
            return;
        }

        let last = self.matches.len() - 1;
        let position = self.selected_position().unwrap_or(0);
        let position = match key {
            KeyCode::Up => position.saturating_sub(1),
            KeyCode::Down => position.saturating_add(1).min(last),
            KeyCode::Home => 0,
            KeyCode::End => last,
            KeyCode::PageUp => position.saturating_sub(self.rows),
            KeyCode::PageDown => position.saturating_add(self.rows).min(last),
            _ => position,
        };

        self.selected = Some(self.matches[position]);
    }

    fn selected_position(&self) -> Option<usize> {
        self.selected
            .and_then(|selected| self.matches.iter().position(|&visit| visit == selected))
    }

    fn update_query(&mut self) -> io::Result<()> {
        if self.query.is_empty() {
            self.search.cancel();
            self.pending = false;
            self.matches = (0..self.snapshot.entries.len()).collect();
            self.selected = self.selected.or_else(|| {
                self.snapshot
                    .entries
                    .iter()
                    .position(|entry| entry.offset == self.snapshot.current)
            });
        } else {
            self.search.query(&self.query)?;
            self.pending = true;
        }

        Ok(())
    }

    fn poll_search(&mut self) -> io::Result<bool> {
        if !self.pending {
            return Ok(false);
        }

        let Some(matches) = self.search.receive()? else {
            return Ok(false);
        };

        self.matches = matches;
        self.pending = false;
        if self.selected_position().is_none() {
            self.selected = self.matches.first().copied();
        }

        Ok(true)
    }

    fn can_confirm(&self) -> bool {
        self.visible && !self.pending && self.selected.is_some()
    }

    fn selection(mut self) -> Selection {
        Selection {
            revision: self.snapshot.revision,
            entry: self
                .snapshot
                .entries
                .remove(self.selected.expect("visible selection")),
        }
    }
}

// Escape terminal controls and format characters, including bidi overrides.
// Display text is never used as a path or a visit identifier.
fn display_path(path: &std::path::Path) -> String {
    path.to_string_lossy().chars().flat_map(|c| {
        if c.is_control() || matches!(c, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}') {
            c.escape_default().to_string().chars().collect::<Vec<_>>()
        } else { vec![c] }
    }).collect()
}

fn draw(frame: &mut Frame, picker: &mut Picker) {
    let area = frame.area();
    picker.resize(area.width, area.height);

    if !picker.visible {
        frame.render_widget(Paragraph::new("Enlarge terminal\nEsc: cancel"), area);
        return;
    }

    let regions = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .split(area);

    let count = if picker.pending {
        format!("…/{}", picker.snapshot.entries.len())
    } else {
        format!("{}/{}", picker.matches.len(), picker.snapshot.entries.len())
    };
    let search_regions = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(count.chars().count() as u16),
    ])
    .split(regions[0]);
    let query = format!("Search: {}", picker.query);
    let query_line = Line::from(query);
    let scroll = query_line
        .width()
        .saturating_sub(search_regions[0].width as usize);
    frame.render_widget(
        Paragraph::new(query_line).scroll((0, scroll.min(u16::MAX as usize) as u16)),
        search_regions[0],
    );
    frame.render_widget(Paragraph::new(count), search_regions[1]);

    let items: Vec<_> = if picker.pending {
        Vec::new()
    } else {
        picker
            .matches
            .iter()
            .map(|&visit| {
                let entry = &picker.snapshot.entries[visit];
                let current = if entry.offset == picker.snapshot.current {
                    '★'
                } else {
                    ' '
                };

                // List clips display text; original visit indices preserve identity.
                ListItem::new(format!(
                    "{current} {:>4} {}",
                    visit + 1,
                    picker.texts[visit]
                ))
            })
            .collect()
    };
    picker.list.select(if picker.pending {
        None
    } else {
        picker.selected_position()
    });

    let direction_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let (back, forward) = if !picker.query.is_empty() {
        (" Search results ", " Ordered by relevance ")
    } else if area.width >= 30 {
        (
            " ↑ BACK (b) · older visits ",
            " ↓ FORWARD (f) · newer visits ",
        )
    } else {
        (" ↑ BACK (b) ", " ↓ FORWARD (f) ")
    };

    let mut block = Block::default()
        .borders(Borders::ALL)
        .title_top(Line::styled(back, direction_style))
        .title_bottom(Line::styled(forward, direction_style));
    if area.width >= 60 {
        block = block.title_top(Line::from(" Directory history ").right_aligned());
    }

    if picker.pending || picker.matches.is_empty() {
        let message = if picker.pending {
            "Searching…"
        } else {
            "No matches"
        };
        frame.render_widget(Paragraph::new(message).block(block), regions[1]);
    } else {
        frame.render_stateful_widget(
            List::new(items)
                .block(block)
                .highlight_symbol("▶ ")
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
            regions[1],
            &mut picker.list,
        );
    }

    let help = if area.width >= 100 {
        "Type to search · Backspace: delete · Ctrl+U: clear · ↑↓ Home/End PgUp/PgDn · Enter: go · Esc: cancel"
    } else if area.width >= 65 {
        "Type: search · Ctrl+U: clear · ↑↓: move · Enter: go · Esc: cancel"
    } else if area.width >= 36 {
        "Type: search · Ctrl+U: clear · Esc: exit"
    } else {
        "Type: search Esc:exit"
    };
    frame.render_widget(Paragraph::new(help), regions[2]);
}

fn restore(tty: &mut File) {
    let _ = execute!(tty, LeaveAlternateScreen, Show);
    let _ = disable_raw_mode();
}

type PanicHook = Arc<dyn Fn(&panic::PanicHookInfo<'_>) + Send + Sync>;

struct TerminalGuard {
    tty: File,
    previous_hook: Option<PanicHook>,
}

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        let tty = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")
            .map_err(|e| io::Error::other(format!("Cannot open controlling terminal: {e}")))?;

        let panic_tty = tty.try_clone()?;
        let previous_hook = Arc::from(panic::take_hook());
        let mut guard = Self {
            tty,
            previous_hook: Some(previous_hook),
        };

        let original = guard.previous_hook.as_ref().unwrap().clone();
        panic::set_hook(Box::new(move |info| {
            if let Ok(mut tty) = panic_tty.try_clone() {
                restore(&mut tty);
            }

            original(info);
        }));

        enable_raw_mode()?;
        execute!(guard.tty, EnterAlternateScreen)?;

        Ok(guard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore(&mut self.tty);

        // Changing the hook while unwinding would itself panic.
        if !std::thread::panicking() {
            if let Some(original) = self.previous_hook.take() {
                panic::set_hook(Box::new(move |info| original(info)));
            }
        }
    }
}

pub fn pick(snapshot: Snapshot) -> io::Result<Option<Selection>> {
    let mut guard = TerminalGuard::enter()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(&mut guard.tty))?;
    let mut picker = Picker::new(snapshot)?;
    let mut dirty = true;

    loop {
        dirty |= picker.poll_search()?;
        if dirty {
            terminal.draw(|frame| draw(frame, &mut picker))?;
            dirty = false;
        }

        // Poll only while work is pending; idle pickers block on terminal input.
        if picker.pending && !event::poll(Duration::from_millis(16))? {
            continue;
        }

        // Crossterm reads /dev/tty when stdin is redirected by the shell.
        match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                match key.code {
                    KeyCode::Esc => return Ok(None),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(None)
                    }
                    KeyCode::Enter if picker.can_confirm() => return Ok(Some(picker.selection())),
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        picker.query.clear();
                        picker.update_query()?;
                    }
                    KeyCode::Backspace => {
                        picker.query.pop();
                        picker.update_query()?;
                    }
                    KeyCode::Char(c)
                        if !c.is_control()
                            && !key
                                .modifiers
                                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                    {
                        picker.query.push(c);
                        picker.update_query()?;
                    }
                    code => picker.navigate(code),
                }

                dirty = true;
            }
            Event::Resize(_, _) => dirty = true,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::Entry;
    use ratatui::backend::TestBackend;

    fn from_paths(paths: &[&str], current: usize) -> Picker {
        Picker::new(Snapshot {
            revision: 7,
            current: current as u64,
            entries: paths
                .iter()
                .enumerate()
                .map(|(index, path)| Entry {
                    offset: index as u64,
                    end: index as u64 + 1,
                    path: path.into(),
                })
                .collect(),
        })
        .unwrap()
    }

    fn picker() -> Picker {
        from_paths(&["/same\t\u{1b}[31m"; 8], 3)
    }

    fn settle(picker: &mut Picker) {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while picker.pending {
            picker.poll_search().unwrap();
            assert!(
                std::time::Instant::now() < deadline,
                "Search worker timed out"
            );
            std::thread::sleep(Duration::from_micros(100));
        }
    }

    fn query(picker: &mut Picker, text: &str) {
        picker.query = text.into();
        picker.update_query().unwrap();
        settle(picker);
    }

    fn screen(terminal: &Terminal<TestBackend>) -> String {
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn fuzzy_relevance_stable_ties_and_literal_text() {
        let mut p = from_paths(
            &[
                "/repo/src",
                "/repo/some/really/cool",
                "/repo/src",
                "/repo/^src$",
            ],
            2,
        );
        query(&mut p, "src");
        assert_eq!(&p.matches[..2], &[0, 2]);
        assert!(p.matches.contains(&1));
        assert_eq!(p.selected, Some(2));

        query(&mut p, "^src$");
        assert_eq!(p.matches, [3]);
        query(&mut p, "!src");
        assert!(p.matches.is_empty());

        let mut p = from_paths(&["/has space", "/hasXspace"], 0);
        query(&mut p, "has space");
        assert_eq!(p.matches, [0]);
    }

    #[test]
    fn smart_case_and_unicode_normalization() {
        let mut p = from_paths(&["/Alpha", "/alpha", "/ALPHA"], 0);
        query(&mut p, "alpha");
        assert_eq!(p.matches.len(), 3);
        query(&mut p, "Alpha");
        assert_eq!(p.matches, [0]);
        query(&mut p, "ALPHA");
        assert_eq!(p.matches, [2]);

        let mut p = from_paths(&["/café", "/cafe", "/CAFÉ", "/žlutý"], 0);
        query(&mut p, "cafe");
        assert_eq!(p.matches.len(), 3);
        query(&mut p, "café");
        assert_eq!(p.matches.len(), 2);
        query(&mut p, "CAFÉ");
        assert_eq!(p.matches, [2]);
        query(&mut p, "zluty");
        assert_eq!(p.matches, [3]);
    }

    #[test]
    fn query_transitions_preserve_visit_identity_and_reject_old_results() {
        let mut p = from_paths(&["/repeat", "/other", "/repeat"], 2);
        p.resize(80, 24);
        assert_eq!(p.matches, [0, 1, 2]);
        assert_eq!(p.selected, Some(2));

        query(&mut p, "repeat");
        assert_eq!(p.selected, Some(2));
        p.navigate(KeyCode::Home);
        query(&mut p, "");
        assert_eq!(p.selected, Some(0));
        assert_eq!(p.matches, [0, 1, 2]);

        query(&mut p, "other");
        assert_eq!(p.selected, Some(1));
        query(&mut p, "");
        assert_eq!(p.selected, Some(1));
        query(&mut p, "missing");
        assert_eq!(p.selected, None);
        assert!(!p.can_confirm());
        p.navigate(KeyCode::Down);
        assert_eq!(p.selected, None);
        query(&mut p, "");
        assert_eq!(p.selected, Some(2));

        for text in ["other", "missing", "repeat"] {
            p.query = text.into();
            p.update_query().unwrap();
            assert!(!p.can_confirm());
        }
        settle(&mut p);
        assert_eq!(p.matches, [0, 2]);
        assert_eq!(p.selected, Some(2));

        p.query = "other".into();
        p.update_query().unwrap();
        query(&mut p, "");
        assert_eq!(p.matches, [0, 1, 2]);
        assert_eq!(p.selected, Some(2));
        let selection = p.selection();
        assert_eq!((selection.revision, selection.entry.offset), (7, 2));
    }

    #[test]
    fn search_uses_full_safe_paths_and_preserves_original_bytes() {
        use std::os::unix::ffi::OsStringExt;

        let long = format!("/{}unique-tail", "segment/".repeat(200));
        let mut p = from_paths(&[&long, "/other"], 1);
        query(&mut p, "unique-tail");
        assert_eq!(p.matches, [0]);
        assert_eq!(p.selection().entry.path, std::path::Path::new(&long));

        let mut p = from_paths(&["/placeholder"], 0);
        let raw = std::ffi::OsString::from_vec(b"/raw-\xff-\x1b".to_vec());
        p.snapshot.entries[0].path = raw.clone().into();
        let mut p = Picker::new(p.snapshot).unwrap();
        query(&mut p, r"\u{1b}");
        assert_eq!(p.matches, [0]);
        assert_eq!(p.selection().entry.path.as_os_str(), raw);
    }

    #[test]
    fn rendering_search_counts_pending_empty_and_small_terminal() {
        let mut p = from_paths(&["/one", "/two", "/two"], 2);
        let mut terminal = Terminal::new(TestBackend::new(80, 8)).unwrap();
        terminal.draw(|f| draw(f, &mut p)).unwrap();
        assert!(screen(&terminal).contains("Search:"));
        assert!(screen(&terminal).contains("3/3"));

        p.query = "two".into();
        p.update_query().unwrap();
        terminal.draw(|f| draw(f, &mut p)).unwrap();
        assert!(screen(&terminal).contains("Searching…"));
        assert!(!screen(&terminal).contains('▶'));
        assert!(!p.can_confirm());

        settle(&mut p);
        terminal.draw(|f| draw(f, &mut p)).unwrap();
        let text = screen(&terminal);
        assert!(text.contains("Search: two"));
        assert!(text.contains("2/3"));
        assert!(text.contains("Search results"));
        assert!(!text.contains("BACK"));
        assert!(!text.contains("FORWARD"));
        assert!(text.contains("★    3 /two"));

        query(&mut p, "missing");
        terminal.draw(|f| draw(f, &mut p)).unwrap();
        assert!(screen(&terminal).contains("No matches"));
        assert!(screen(&terminal).contains("0/3"));
        assert!(!screen(&terminal).contains('▶'));
        assert_eq!(p.list.selected(), None);

        query(&mut p, "");
        terminal.backend_mut().resize(20, 4);
        terminal.draw(|f| draw(f, &mut p)).unwrap();
        assert!(!p.can_confirm());
        assert!(screen(&terminal).contains("Enlarge terminal"));

        terminal.backend_mut().resize(20, 5);
        terminal.draw(|f| draw(f, &mut p)).unwrap();
        assert!(p.can_confirm());
        assert_eq!(p.rows, 1);
    }

    #[test]
    #[ignore = "manual release benchmark; no timing assertions"]
    fn benchmark_search_10000_visits() {
        let paths: Vec<_> = (0..10_000)
            .map(|i| {
                format!(
                    "/workspace/project-{i:05}/src/module-{}/component.rs",
                    i % 137
                )
            })
            .collect();
        let paths: Vec<_> = paths.iter().map(String::as_str).collect();
        let started = std::time::Instant::now();
        let mut p = from_paths(&paths, 5_000);
        let setup = started.elapsed();
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        let mut measurements = Vec::new();

        for _ in 0..10 {
            for text in ["src", "prj42", "component", "module-13", "missing", ""] {
                let started = std::time::Instant::now();
                query(&mut p, text);
                let matched = started.elapsed();
                terminal.draw(|f| draw(f, &mut p)).unwrap();
                measurements.push((
                    matched.as_secs_f64() * 1000.0,
                    started.elapsed().as_secs_f64() * 1000.0,
                ));
            }
        }

        measurements.sort_by(|a, b| a.1.total_cmp(&b.1));
        eprintln!("10,000 visits: setup={setup:?}; query+draw p50={:.2}ms p95={:.2}ms max={:.2}ms (60 edits); matching at median={:.2}ms",
            measurements[30].1, measurements[57].1, measurements[59].1, measurements[30].0);
    }

    #[test]
    fn terminal_guard_restores_after_errors_and_panics() {
        if let Ok(mode) = std::env::var("DIRSTORY_TEST_GUARD") {
            let result = panic::catch_unwind(|| -> io::Result<()> {
                let _guard = TerminalGuard::enter()?;
                assert!(crossterm::terminal::is_raw_mode_enabled()?);

                if mode == "panic" {
                    panic!("terminal cleanup test");
                }

                Err(io::Error::other("failure after terminal activation"))
            });

            assert!(!crossterm::terminal::is_raw_mode_enabled().unwrap());
            if mode == "panic" {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }

            return;
        }

        // Run the same test in a child with a real controlling terminal. Keep
        // fault injection entirely in tests, rather than in the picker protocol.
        let script = r#"
import fcntl, os, pty, subprocess, sys, termios
master, slave = pty.openpty()
before = termios.tcgetattr(slave)
def setup():
    os.setsid()
    fcntl.ioctl(0, termios.TIOCSCTTY, 0)
try:
    child = subprocess.run([sys.argv[1], '--exact',
        'picker::tests::terminal_guard_restores_after_errors_and_panics', '--nocapture'],
        stdin=slave, capture_output=True, preexec_fn=setup, timeout=10,
        env=dict(os.environ, DIRSTORY_TEST_GUARD=sys.argv[2]))
    assert child.returncode == 0, (child.stdout, child.stderr)
    assert termios.tcgetattr(slave) == before, 'terminal flags leaked'
    output = os.read(master, 4096)
    assert b'\x1b[?1049h' in output and b'\x1b[?1049l' in output, output
finally:
    os.close(slave)
    os.close(master)
"#;

        for mode in ["error", "panic"] {
            let output = std::process::Command::new("python3")
                .arg("-c")
                .arg(script)
                .arg(std::env::current_exe().unwrap())
                .arg(mode)
                .output()
                .unwrap();

            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    #[test]
    fn movement_resize_and_visit_identity() {
        let mut p = picker();
        assert_eq!(p.selected, Some(3));

        p.resize(40, 7);
        p.navigate(KeyCode::PageDown);
        assert_eq!(p.selected, Some(6));

        p.navigate(KeyCode::Down);
        p.navigate(KeyCode::Down);
        assert_eq!(p.selected, Some(7));

        p.navigate(KeyCode::PageUp);
        assert_eq!(p.selected, Some(4));

        p.resize(80, 6);
        assert_eq!(p.selected, Some(4));

        p.navigate(KeyCode::Home);
        p.navigate(KeyCode::Up);
        assert_eq!(p.selected, Some(0));

        p.navigate(KeyCode::End);
        let s = p.selection();
        assert_eq!((s.revision, s.entry.offset), (7, 7));
        assert_eq!(s.entry.path, std::path::Path::new("/same\t\u{1b}[31m"));
    }

    #[test]
    fn rendering_scroll_controls_and_small_terminal() {
        let mut p = picker();
        let mut terminal = Terminal::new(TestBackend::new(40, 7)).unwrap();
        terminal.draw(|f| draw(f, &mut p)).unwrap();

        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("★    4 /same\\t\\u{1b}[31m"));
        assert!(!text.contains('\u{1b}'));
        assert!(text.contains("↑ BACK (b) · older visits"));
        assert!(text.contains("↓ FORWARD (f) · newer visits"));
        assert!(text.contains("▶ ★"));

        let selected = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .any(|c| c.modifier.contains(Modifier::REVERSED));
        assert!(selected);

        p.navigate(KeyCode::Up);
        terminal.draw(|f| draw(f, &mut p)).unwrap();
        let current = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .find(|cell| cell.symbol() == "★")
            .unwrap();
        assert!(!current.modifier.contains(Modifier::REVERSED));

        p.navigate(KeyCode::Down);
        terminal.backend_mut().resize(19, 3);
        terminal.draw(|f| draw(f, &mut p)).unwrap();
        assert!(!p.visible);
        assert_eq!(p.selected, Some(3));

        terminal.backend_mut().resize(20, 5);
        terminal.draw(|f| draw(f, &mut p)).unwrap();
        assert!(p.visible);
    }
}
