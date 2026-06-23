//! Interactive terminal UI: terminal lifecycle, event loop, and action handling.

mod events;
mod ui;

use std::io::{self, Stdout};
use std::time::Duration;

use anyhow::Result;
use crossbeam_channel::Receiver;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::widgets::TableState;
use ratatui::Terminal;

use crate::delete;
use crate::model::AppState;
use crate::scanner::ScanMsg;
use events::{Action, Mode};

/// Run the full-screen UI, draining `rx` for live scan results. Returns the
/// number of bytes reclaimed during the session.
pub fn run(mut app: AppState, rx: Receiver<ScanMsg>) -> Result<u64> {
    let mut terminal = setup_terminal()?;
    let result = event_loop(&mut terminal, &mut app, rx);
    restore_terminal(&mut terminal)?;
    result?;
    Ok(app.reclaimed)
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    // Make sure a panic doesn't leave the user's terminal in raw mode.
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        prev(info);
    }));

    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn restore_terminal<B: Backend + io::Write>(terminal: &mut Terminal<B>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn event_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut AppState,
    rx: Receiver<ScanMsg>,
) -> Result<()> {
    let mut mode = Mode::Normal;
    let mut table_state = TableState::default();
    let mut status: Option<String> = None;
    let mut pending: Vec<usize> = Vec::new();

    loop {
        // Drain everything the scanner has produced since the last frame.
        for msg in rx.try_iter() {
            match msg {
                ScanMsg::Found(c) => app.push(c),
                ScanMsg::Sized { id, bytes, mtime } => app.set_size(id, bytes, mtime),
                ScanMsg::ScanDone => app.scanning = false,
                ScanMsg::SizingDone => app.sizing = false,
            }
        }
        app.clamp_cursor();

        terminal.draw(|f| ui::draw(f, app, &mut table_state, mode, &status, &pending))?;

        // Poll with a timeout so the spinner animates and live results appear
        // even while the user isn't pressing keys.
        if event::poll(Duration::from_millis(120))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    let action = events::map(mode, key);
                    if handle(action, app, &mut mode, &mut status, &mut pending) {
                        break;
                    }
                }
            }
        } else {
            app.spinner = app.spinner.wrapping_add(1);
        }
    }
    Ok(())
}

/// Apply an action. Returns `true` when the app should quit.
fn handle(
    action: Action,
    app: &mut AppState,
    mode: &mut Mode,
    status: &mut Option<String>,
    pending: &mut Vec<usize>,
) -> bool {
    *status = None;

    match action {
        Action::Quit => return true,

        Action::Up => app.cursor = app.cursor.saturating_sub(1),
        Action::Down => {
            app.cursor += 1;
            app.clamp_cursor();
        }
        Action::Top => app.cursor = 0,
        Action::Bottom => app.cursor = app.found_count().saturating_sub(1),

        Action::ToggleSelect => {
            if let Some(i) = app.focused_index() {
                if !app.get(i).deleted {
                    let sel = app.get(i).selected;
                    app.candidate_mut(i).selected = !sel;
                }
            }
            app.cursor += 1;
            app.clamp_cursor();
        }
        Action::SelectAll => app.select_all_in_view(),
        Action::ClearSelection => app.clear_selection(),

        Action::RequestDelete => {
            let targets = app.deletion_targets();
            if targets.is_empty() {
                *status = Some("Nothing to delete.".into());
            } else {
                *pending = targets;
                *mode = Mode::Confirm;
            }
        }
        Action::ConfirmYes => {
            perform_deletion(app, pending, status);
            pending.clear();
            *mode = Mode::Normal;
        }
        Action::ConfirmNo => {
            pending.clear();
            *mode = Mode::Normal;
        }

        Action::CycleSort => app.sort = app.sort.next(),

        Action::StartFilter => *mode = Mode::Filter,
        Action::FilterInput(c) => {
            app.filter.push(c);
            app.cursor = 0;
        }
        Action::FilterBackspace => {
            app.filter.pop();
        }
        Action::FilterDone => *mode = Mode::Normal,
        Action::FilterClear => {
            app.filter.clear();
            *mode = Mode::Normal;
        }

        Action::ToggleHelp => {
            *mode = if *mode == Mode::Help {
                Mode::Normal
            } else {
                Mode::Help
            }
        }

        Action::Nothing => {}
    }
    false
}

fn perform_deletion(app: &mut AppState, pending: &[usize], status: &mut Option<String>) {
    let root = app.root.clone();
    let dry_run = app.dry_run;
    let mut first_error: Option<String> = None;

    for &idx in pending {
        if app.get(idx).deleted {
            continue;
        }
        let path = app.get(idx).path.clone();
        let size = app.get(idx).size.unwrap_or(0);

        let ok = if dry_run {
            true
        } else {
            match delete::remove(&root, &path) {
                Ok(()) => true,
                Err(e) => {
                    if first_error.is_none() {
                        first_error = Some(format!("{}: {e}", path.display()));
                    }
                    false
                }
            }
        };

        if ok {
            {
                let c = app.candidate_mut(idx);
                c.deleted = true;
                c.selected = false;
            }
            app.reclaimed += size;
        }
    }

    *status = first_error;
}
