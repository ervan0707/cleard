//! All ratatui rendering for the interactive screen.

use std::time::SystemTime;

use humansize::{format_size, BINARY};
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use super::events::Mode;
use crate::model::AppState;

const SPINNER: [&str; 4] = ["|", "/", "-", "\\"];
const BAR_WIDTH: usize = 12;

pub fn draw(
    f: &mut Frame,
    app: &AppState,
    table_state: &mut TableState,
    mode: Mode,
    status: &Option<String>,
    pending: &[usize],
) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // header
        Constraint::Min(1),    // table
        Constraint::Length(2), // footer
    ])
    .split(f.area());

    draw_header(f, chunks[0], app);
    draw_table(f, chunks[1], app, table_state);
    draw_footer(f, chunks[2], app, mode, status);

    match mode {
        Mode::Confirm => draw_confirm(f, app, pending),
        Mode::Help => draw_help(f),
        _ => {}
    }
}

fn draw_header(f: &mut Frame, area: Rect, app: &AppState) {
    let spin = if app.scanning {
        format!(" {} scanning", SPINNER[app.spinner % SPINNER.len()])
    } else if app.sizing {
        format!(" {} sizing", SPINNER[app.spinner % SPINNER.len()])
    } else {
        " done".to_string()
    };
    let line = Line::from(vec![
        Span::styled(" cleard ", Style::new().fg(Color::Black).bg(Color::Cyan).bold()),
        Span::raw(format!(" {}", app.root.display())),
        Span::styled(spin, Style::new().fg(Color::Yellow)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn draw_table(f: &mut Frame, area: Rect, app: &AppState, table_state: &mut TableState) {
    let view = app.view_indices();
    if view.is_empty() {
        let msg = if app.scanning {
            "Scanning for reclaimable directories…"
        } else {
            "Nothing to reclaim here."
        };
        let p = Paragraph::new(msg)
            .alignment(Alignment::Center)
            .style(Style::new().fg(Color::DarkGray));
        f.render_widget(p, area);
        return;
    }

    let max_size = view
        .iter()
        .filter_map(|&i| app.get(i).size)
        .max()
        .unwrap_or(1)
        .max(1);

    let now = SystemTime::now();
    let rows: Vec<Row> = view
        .iter()
        .map(|&i| {
            let c = app.get(i);
            let mark = if c.selected { "▣" } else { " " };
            let size_text = match c.size {
                Some(b) => format_size(b, BINARY),
                None => "…".to_string(),
            };
            let bar = size_bar(c.size.unwrap_or(0), max_size);
            let age = age_text(c.mtime, now);

            let mut style = Style::new();
            if c.deleted {
                style = style.add_modifier(Modifier::CROSSED_OUT).fg(Color::DarkGray);
            } else if c.selected {
                style = style.fg(Color::LightRed).add_modifier(Modifier::BOLD);
            }

            Row::new(vec![
                Cell::from(mark),
                Cell::from(c.ecosystem.clone()).style(Style::new().fg(eco_color(&c.ecosystem))),
                Cell::from(Line::from(size_text).alignment(Alignment::Right)),
                Cell::from(Span::styled(bar, Style::new().fg(Color::Blue))),
                Cell::from(age),
                Cell::from(c.path.display().to_string()),
            ])
            .style(style)
        })
        .collect();

    let header = Row::new(vec!["", "Ecosystem", "Size", "", "Age", "Path"])
        .style(Style::new().fg(Color::Gray).add_modifier(Modifier::BOLD));

    let widths = [
        Constraint::Length(2),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(BAR_WIDTH as u16),
        Constraint::Length(6),
        Constraint::Min(10),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol("");

    table_state.select(Some(app.cursor.min(view.len().saturating_sub(1))));
    f.render_stateful_widget(table, area, table_state);
}

fn draw_footer(f: &mut Frame, area: Rect, app: &AppState, mode: Mode, status: &Option<String>) {
    let stats = Line::from(vec![
        Span::styled(format!(" {} found", app.found_count()), Style::new().fg(Color::Cyan)),
        Span::raw("  "),
        Span::styled(
            format!("{} reclaimable", format_size(app.reclaimable(), BINARY)),
            Style::new().fg(Color::Green),
        ),
        Span::raw("  "),
        Span::styled(
            format!("{} reclaimed", format_size(app.reclaimed, BINARY)),
            Style::new().fg(Color::LightGreen).bold(),
        ),
        Span::raw("  "),
        Span::styled(format!("sort:{}", app.sort.label()), Style::new().fg(Color::Magenta)),
        if app.dry_run {
            Span::styled("  [dry-run]", Style::new().fg(Color::Yellow).bold())
        } else {
            Span::raw("")
        },
    ]);

    let hint_line = if app.deleting {
        Line::from(Span::styled(
            format!(" {} deleting… (UI stays responsive)", SPINNER[app.spinner % SPINNER.len()]),
            Style::new().fg(Color::Yellow).bold(),
        ))
    } else if mode == Mode::Filter {
        Line::from(vec![
            Span::styled("/", Style::new().fg(Color::Yellow)),
            Span::raw(app.filter.clone()),
            Span::styled("▏", Style::new().fg(Color::Yellow)),
            Span::styled("  (enter: apply, esc: clear)", Style::new().fg(Color::DarkGray)),
        ])
    } else if let Some(s) = status {
        Line::from(Span::styled(s.clone(), Style::new().fg(Color::Red)))
    } else {
        Line::from(Span::styled(
            "↑↓/jk move · space select · d delete · s sort · / filter · ? help · q quit",
            Style::new().fg(Color::DarkGray),
        ))
    };

    let p = Paragraph::new(vec![stats, hint_line]);
    f.render_widget(p, area);
}

fn draw_confirm(f: &mut Frame, app: &AppState, pending: &[usize]) {
    let total: u64 = pending.iter().filter_map(|&i| app.get(i).size).sum();
    let verb = if app.dry_run { "Simulate deleting" } else { "Delete" };
    let body = vec![
        Line::from(Span::styled(
            format!("{} {} director{}?", verb, pending.len(), if pending.len() == 1 { "y" } else { "ies" }),
            Style::new().bold(),
        )),
        Line::from(format!("Total size: {}", format_size(total, BINARY))),
        Line::from(""),
        Line::from(Span::styled(
            if app.dry_run {
                "y: confirm (no files removed)   n/esc: cancel"
            } else {
                "y: permanently delete   n/esc: cancel"
            },
            Style::new().fg(Color::Yellow),
        )),
    ];
    let area = centered_rect(f.area(), 54, 7);
    f.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Confirm ")
        .border_style(Style::new().fg(Color::Red));
    f.render_widget(Paragraph::new(body).block(block).wrap(Wrap { trim: true }), area);
}

fn draw_help(f: &mut Frame) {
    let lines = vec![
        Line::from(Span::styled("cleard — keys", Style::new().bold())),
        Line::from(""),
        Line::from("  ↑/k, ↓/j    move cursor"),
        Line::from("  g / G       jump to top / bottom"),
        Line::from("  space       toggle selection"),
        Line::from("  a / c       select all / clear selection"),
        Line::from("  d / Del     delete selected (or focused)"),
        Line::from("  Enter       delete focused"),
        Line::from("  s           cycle sort (size/age/path)"),
        Line::from("  /           filter by path or ecosystem"),
        Line::from("  ? / Esc     close this help"),
        Line::from("  q           quit"),
    ];
    let area = centered_rect(f.area(), 50, lines.len() as u16 + 2);
    f.render_widget(Clear, area);
    let block = Block::default().borders(Borders::ALL).title(" Help ");
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn size_bar(size: u64, max: u64) -> String {
    let filled = ((size as f64 / max as f64) * BAR_WIDTH as f64).round() as usize;
    let filled = filled.min(BAR_WIDTH);
    let mut s = String::with_capacity(BAR_WIDTH);
    for _ in 0..filled {
        s.push('█');
    }
    for _ in filled..BAR_WIDTH {
        s.push('░');
    }
    s
}

fn age_text(mtime: Option<SystemTime>, now: SystemTime) -> String {
    let Some(t) = mtime else { return "—".into() };
    let secs = now.duration_since(t).map(|d| d.as_secs()).unwrap_or(0);
    if secs < 60 {
        "now".into()
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86_400 {
        format!("{}h", secs / 3600)
    } else if secs < 86_400 * 7 {
        format!("{}d", secs / 86_400)
    } else if secs < 86_400 * 365 {
        format!("{}w", secs / (86_400 * 7))
    } else {
        format!("{}y", secs / (86_400 * 365))
    }
}

fn eco_color(eco: &str) -> Color {
    match eco {
        "Node" => Color::Green,
        "Rust" => Color::Red,
        "Python" => Color::Blue,
        "Go" => Color::Cyan,
        "Maven" | "Gradle" => Color::Yellow,
        ".NET" => Color::Magenta,
        _ => Color::Gray,
    }
}

/// A rectangle of fixed width/height centred within `area`.
fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let h = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .split(area);
    let v = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .split(h[0]);
    v[0]
}
