//! PAC1 Dashboard — TUI split view: heatmap + log viewer.
//!
//! Left: compact task×run heatmap (block chars)
//! Right: full log for selected task+run
//!
//! Usage: cargo run --bin pac1-dash

use std::{io, path::Path, collections::HashMap};
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    execute,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap},
    Terminal,
};

const TASK_IDS: &[&str] = &[
    "t01","t02","t03","t04","t05","t06","t07","t08","t09","t10",
    "t11","t12","t13","t14","t15","t16","t17","t18","t19","t20",
    "t21","t22","t23","t24","t25","t26","t27","t28","t29","t30",
    "t31","t32","t33","t34","t35","t36","t37","t38","t39","t40",
    "t41","t42","t43",
];

struct RunData {
    label: String,
    tasks: HashMap<String, f32>,
}

fn parse_log_md_runs() -> Vec<RunData> {
    let content = std::fs::read_to_string("LOG.md").unwrap_or_default();
    let mut runs = Vec::new();
    for line in content.lines() {
        if line.starts_with("| ") && line.contains('`') && line.contains('%') {
            let cols: Vec<&str> = line.split('|').map(|s| s.trim()).collect();
            if cols.len() >= 6 {
                let date = cols[1]; // "04-09"
                let day = date.split('-').last().unwrap_or(date).trim_start_matches('0');
                let score_pct = cols[4].split('(').next().unwrap_or("").trim();
                let failures_str = cols[5];
                let failures: Vec<&str> = failures_str.split(", ")
                    .filter(|s| s.starts_with('t'))
                    .map(|s| s.split_whitespace().next().unwrap_or(s))
                    .collect();
                let mut tasks = HashMap::new();
                for tid in TASK_IDS {
                    tasks.insert(tid.to_string(), if failures.contains(tid) { 0.0 } else { 1.0 });
                }
                runs.push(RunData { label: format!("{}·{}", day, score_pct), tasks });
            }
        }
    }
    runs
}

fn parse_run_files() -> Vec<RunData> {
    let dir = Path::new("benchmarks/runs");
    if !dir.exists() { return Vec::new(); }
    let mut entries: Vec<_> = std::fs::read_dir(dir).into_iter().flatten().flatten()
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
        .collect();
    entries.sort_by_key(|e| e.file_name());
    entries.iter().filter_map(|e| {
        let content = std::fs::read_to_string(e.path()).ok()?;
        let fname = e.path().file_stem()?.to_str()?.to_string();
        let parts: Vec<&str> = fname.split("__").collect();
        let day = parts.first().and_then(|d| d.split('-').last()).unwrap_or("?");
        let _prov = parts.get(1).unwrap_or(&"?");
        let mut tasks = HashMap::new();
        for line in content.lines() {
            if line.starts_with("| t") {
                let cols: Vec<&str> = line.split('|').map(|s| s.trim()).collect();
                if cols.len() >= 3 {
                    tasks.insert(cols[1].to_string(), cols[2].parse().unwrap_or(-1.0));
                }
            }
        }
        let pass = tasks.values().filter(|&&v| v >= 1.0).count();
        let total = tasks.len();
        Some(RunData { label: format!("{}·{}", day, if total > 0 { format!("{}", pass) } else { "?".into() }), tasks })
    }).collect()
}

fn load_runs() -> Vec<RunData> {
    let file_runs = parse_run_files();
    if !file_runs.is_empty() { return file_runs; }
    parse_log_md_runs()
}

/// Find and render log for task+run combination.
/// Priority: /tmp logs → dump files (pipeline + inbox + contacts) → BitGN URL.
fn find_log(task: &str, _run_idx: usize) -> String {
    // 1. Try /tmp logs (from recent runs)
    for suffix in &["", "v2", "v3", "-retry"] {
        let path = if suffix.is_empty() {
            format!("/tmp/evolve-{}.log", task)
        } else {
            format!("/tmp/{}{}.log", task, suffix)
        };
        if let Ok(content) = std::fs::read_to_string(&path) {
            if !content.is_empty() { return content; }
        }
    }

    // 2. Try benchmarks/tasks/{task}/ dump directory
    let task_dir = format!("benchmarks/tasks/{}", task);
    if let Ok(entries) = std::fs::read_dir(&task_dir) {
        let mut dirs: Vec<_> = entries.flatten().filter(|e| e.path().is_dir()).collect();
        dirs.sort_by_key(|e| e.file_name());
        if let Some(latest) = dirs.last() {
            let dir = latest.path();
            let mut out = String::new();

            // Header: trial ID
            out.push_str(&format!("━━━ {} — {} ━━━\n\n", task, dir.file_name().unwrap_or_default().to_string_lossy()));

            // BitGN URL
            if let Ok(url) = std::fs::read_to_string(dir.join("bitgn_log.url")) {
                out.push_str(&format!("🔗 BitGN: {}\n\n", url.trim()));
            }

            // Pipeline info
            if let Ok(content) = std::fs::read_to_string(dir.join("pipeline.txt")) {
                out.push_str("── Pipeline ──\n");
                out.push_str(&content);
                out.push_str("\n\n");
            }

            // Inbox files (show content with classification headers)
            let mut inbox_files: Vec<_> = std::fs::read_dir(&dir)
                .into_iter().flatten().flatten()
                .filter(|e| e.file_name().to_string_lossy().starts_with("inbox_"))
                .collect();
            inbox_files.sort_by_key(|e| e.file_name());
            if !inbox_files.is_empty() {
                out.push_str("── Inbox ──\n");
                for f in &inbox_files {
                    if let Ok(content) = std::fs::read_to_string(f.path()) {
                        out.push_str(&content);
                        out.push_str("\n---\n");
                    }
                }
                out.push('\n');
            }

            // Contacts summary
            if let Ok(content) = std::fs::read_to_string(dir.join("contacts.txt")) {
                let lines: Vec<&str> = content.lines().take(15).collect();
                out.push_str(&format!("── Contacts ({} lines) ──\n", content.lines().count()));
                out.push_str(&lines.join("\n"));
                out.push_str("\n\n");
            }

            // Accounts summary
            if let Ok(content) = std::fs::read_to_string(dir.join("accounts.txt")) {
                let lines: Vec<&str> = content.lines().take(15).collect();
                out.push_str(&format!("── Accounts ({} lines) ──\n", content.lines().count()));
                out.push_str(&lines.join("\n"));
                out.push_str("\n\n");
            }

            // Tree
            if let Ok(content) = std::fs::read_to_string(dir.join("tree.txt")) {
                out.push_str("── Tree ──\n");
                out.push_str(&content);
                out.push_str("\n\n");
            }

            // AGENTS.md
            if let Ok(content) = std::fs::read_to_string(dir.join("agents.md")) {
                let lines: Vec<&str> = content.lines().take(20).collect();
                out.push_str("── AGENTS.md ──\n");
                out.push_str(&lines.join("\n"));
                out.push('\n');
            }

            if !out.is_empty() { return out; }
        }
    }

    format!("No data for {}\n\nRun with DUMP_TRIAL=1:\n  DUMP_TRIAL=1 cargo run -- --provider nemotron --task {}", task, task)
}

fn run_app() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut table_state = TableState::default();
    table_state.select(Some(0));
    let mut col_select: usize = 0;
    let mut log_scroll: u16 = 0;

    let runs = load_runs();

    loop {
        let selected_task = table_state.selected().unwrap_or(0);
        let task_id = TASK_IDS[selected_task];
        let log_content = find_log(task_id, col_select);

        terminal.draw(|f| {
            // Split: left 55% heatmap, right 45% log
            let main = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
                .split(f.area());

            // ── LEFT: Heatmap ──
            let left = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(5), Constraint::Length(1)])
                .split(main[0]);

            // Title
            let title = Paragraph::new(format!(" PAC1 — {} runs | q=quit ↑↓=task ←→=run", runs.len()))
                .style(Style::default().fg(Color::Cyan));
            f.render_widget(title, left[0]);

            // Header row
            let max_cols = ((left[1].width as usize).saturating_sub(8)) / 4;
            let end = runs.len().min(col_select.saturating_add(max_cols));
            let start = end.saturating_sub(max_cols);
            let visible: Vec<&RunData> = runs[start..end].iter().collect();

            let mut hdr = vec![Cell::from("").style(Style::default())];
            for (i, run) in visible.iter().enumerate() {
                let lbl: String = run.label.chars().take(4).collect();
                let style = if start + i == col_select {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                hdr.push(Cell::from(lbl.to_string()).style(style));
            }
            hdr.push(Cell::from("%").style(Style::default().fg(Color::DarkGray)));

            let rows: Vec<Row> = TASK_IDS.iter().enumerate().map(|(idx, &tid)| {
                let tid_style = if idx == selected_task {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                let mut cells = vec![Cell::from(tid).style(tid_style)];

                let mut pass = 0u32;
                let mut total = 0u32;
                for (i, run) in visible.iter().enumerate() {
                    let score = run.tasks.get(tid).copied().unwrap_or(-1.0);
                    let is_selected = idx == selected_task && start + i == col_select;
                    let (ch, fg, bg) = if score < 0.0 {
                        ("·", Color::DarkGray, Color::Reset)
                    } else if score >= 1.0 {
                        pass += 1; total += 1;
                        if is_selected { ("█", Color::Black, Color::Green) }
                        else { ("█", Color::Green, Color::Reset) }
                    } else {
                        total += 1;
                        if is_selected { ("█", Color::Black, Color::Red) }
                        else { ("█", Color::Red, Color::Reset) }
                    };
                    cells.push(Cell::from(ch).style(Style::default().fg(fg).bg(bg)));
                }

                let rate = if total > 0 { pass * 100 / total } else { 0 };
                let rate_color = match rate {
                    100 => Color::Green,
                    75..=99 => Color::Yellow,
                    50..=74 => Color::Magenta,
                    _ => Color::Red,
                };
                cells.push(Cell::from(format!("{}", rate)).style(Style::default().fg(rate_color)));
                Row::new(cells)
            }).collect();

            let mut widths = vec![Constraint::Length(4)];
            for _ in &visible { widths.push(Constraint::Length(2)); }
            widths.push(Constraint::Length(4));

            let table = Table::new(rows, widths)
                .header(Row::new(hdr))
                .block(Block::default().borders(Borders::RIGHT))
                .row_highlight_style(Style::default());
            f.render_stateful_widget(table, left[1], &mut table_state);

            // Bottom status
            let status = Paragraph::new(format!(" {} col:{}", task_id, col_select))
                .style(Style::default().fg(Color::DarkGray));
            f.render_widget(status, left[2]);

            // ── RIGHT: Log viewer ──
            let log_lines: Vec<Line> = log_content.lines().skip(log_scroll as usize).take(main[1].height as usize - 2).map(|l| {
                let style = if l.contains("Score: 1") { Style::default().fg(Color::Green) }
                    else if l.contains("Score: 0") { Style::default().fg(Color::Red) }
                    else if l.contains("⚠") || l.contains("⛔") { Style::default().fg(Color::Yellow) }
                    else if l.contains("🎯") || l.contains("Skill") { Style::default().fg(Color::Cyan) }
                    else if l.contains("→ ") { Style::default().fg(Color::Blue) }
                    else { Style::default().fg(Color::White) };
                Line::from(Span::styled(l, style))
            }).collect();

            let total_lines = log_content.lines().count();
            let log_widget = Paragraph::new(log_lines)
                .block(Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {} log ({}/{} lines) PgUp/PgDn ", task_id, log_scroll, total_lines))
                )
                .wrap(Wrap { trim: false });
            f.render_widget(log_widget, main[1]);
        })?;

        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Down | KeyCode::Char('j') => {
                        let i = table_state.selected().unwrap_or(0);
                        table_state.select(Some((i + 1).min(TASK_IDS.len() - 1)));
                        log_scroll = 0;
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        let i = table_state.selected().unwrap_or(0);
                        table_state.select(Some(i.saturating_sub(1)));
                        log_scroll = 0;
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        if col_select + 1 < runs.len() { col_select += 1; }
                        log_scroll = 0;
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        col_select = col_select.saturating_sub(1);
                        log_scroll = 0;
                    }
                    KeyCode::PageDown => { log_scroll = log_scroll.saturating_add(20); }
                    KeyCode::PageUp => { log_scroll = log_scroll.saturating_sub(20); }
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

fn main() {
    if let Err(e) = run_app() {
        eprintln!("Dashboard error: {}", e);
    }
}
