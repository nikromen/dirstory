//! Snapshot selection, drawing, and terminal ownership are kept independent.
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
use std::{
    fs::{File, OpenOptions},
    io, panic,
    sync::Arc,
};

struct Picker {
    snapshot: Snapshot,
    selected: usize,
    rows: usize,
    visible: bool,
    list: ListState,
}

impl Picker {
    fn new(snapshot: Snapshot) -> Self {
        let selected = snapshot
            .entries
            .iter()
            .position(|e| e.offset == snapshot.current)
            .expect("validated snapshot has a current visit");

        Self {
            snapshot,
            selected,
            rows: 1,
            visible: false,
            list: ListState::default(),
        }
    }

    fn resize(&mut self, width: u16, height: u16) {
        self.visible = width >= 20 && height >= 4;
        self.rows = usize::from(height.saturating_sub(3)).max(1);
    }

    fn navigate(&mut self, key: KeyCode) {
        let last = self.snapshot.entries.len() - 1;

        self.selected = match key {
            KeyCode::Up => self.selected.saturating_sub(1),
            KeyCode::Down => self.selected.saturating_add(1).min(last),
            KeyCode::Home => 0,
            KeyCode::End => last,
            KeyCode::PageUp => self.selected.saturating_sub(self.rows),
            KeyCode::PageDown => self.selected.saturating_add(self.rows).min(last),
            _ => self.selected,
        };
    }

    fn selection(mut self) -> Selection {
        Selection {
            revision: self.snapshot.revision,
            entry: self.snapshot.entries.remove(self.selected),
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
        frame.render_widget(Paragraph::new("Enlarge terminal\nEsc/q: cancel"), area);
        return;
    }

    let regions = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).split(area);

    let items: Vec<_> = picker
        .snapshot
        .entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let current = if entry.offset == picker.snapshot.current {
                '★'
            } else {
                ' '
            };

            // List truncates each single-line item to the available display width.
            ListItem::new(format!(
                "{current} {:>4} {}",
                i + 1,
                display_path(&entry.path)
            ))
        })
        .collect();
    picker.list.select(Some(picker.selected));

    let direction_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let (back, forward) = if area.width >= 30 {
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

    frame.render_stateful_widget(
        List::new(items)
            .block(block)
            .highlight_symbol("▶ ")
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
        regions[0],
        &mut picker.list,
    );

    let help = if area.width >= 75 {
        "↑↓ Home/End PgUp/PgDn · Enter: go · Esc/q: cancel · ★ current · ▶ selected"
    } else if area.width >= 36 {
        "Enter: go · Esc/q: cancel · ★ current"
    } else {
        "Enter: go q/Esc: exit"
    };
    frame.render_widget(Paragraph::new(help), regions[1]);
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
    let mut picker = Picker::new(snapshot);

    loop {
        terminal.draw(|frame| draw(frame, &mut picker))?;

        // Crossterm reads /dev/tty when stdin is redirected by the shell.
        match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => return Ok(None),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(None)
                }
                KeyCode::Enter if picker.visible => return Ok(Some(picker.selection())),
                code => picker.navigate(code),
            },
            Event::Resize(_, _) => {}
            _ => continue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::Entry;
    use ratatui::backend::TestBackend;

    fn picker() -> Picker {
        Picker::new(Snapshot {
            revision: 7,
            current: 3,
            entries: (0..8)
                .map(|offset| Entry {
                    offset,
                    end: offset + 1,
                    path: "/same\t\u{1b}[31m".into(),
                })
                .collect(),
        })
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
        assert_eq!(p.selected, 3);

        p.resize(40, 6);
        p.navigate(KeyCode::PageDown);
        assert_eq!(p.selected, 6);

        p.navigate(KeyCode::Down);
        p.navigate(KeyCode::Down);
        assert_eq!(p.selected, 7);

        p.navigate(KeyCode::PageUp);
        assert_eq!(p.selected, 4);

        p.resize(80, 5);
        assert_eq!(p.selected, 4);

        p.navigate(KeyCode::Home);
        p.navigate(KeyCode::Up);
        assert_eq!(p.selected, 0);

        p.navigate(KeyCode::End);
        let s = p.selection();
        assert_eq!((s.revision, s.entry.offset), (7, 7));
        assert_eq!(s.entry.path, std::path::Path::new("/same\t\u{1b}[31m"));
    }

    #[test]
    fn rendering_scroll_controls_and_small_terminal() {
        let mut p = picker();
        let mut terminal = Terminal::new(TestBackend::new(40, 6)).unwrap();
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
        assert_eq!(p.selected, 3);

        terminal.backend_mut().resize(20, 4);
        terminal.draw(|f| draw(f, &mut p)).unwrap();
        assert!(p.visible);
    }
}
