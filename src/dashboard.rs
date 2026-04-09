//! PAC1 Dashboard — TUI split view: heatmap + log viewer.
//!
//! Data: benchmarks/tasks/{task_id}/{trial_id}/score.txt + dump files
//! Left: compact task×run heatmap
//! Right: trial dump (pipeline, inbox, contacts, tree)
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

/// One trial result for a task.
struct Trial {
    trial_id: String,
    score: f32,     // -1 = no score yet
    detail: String, // score_detail lines
}

/// All trials grouped by task.
struct TaskData {
    trials: Vec<Trial>,
}

/// Load all data from benchmarks/tasks/.
fn load_task_data() -> HashMap<String, TaskData> {
    let mut data: HashMap<String, TaskData> = HashMap::new();
    let base = Path::new("benchmarks/tasks");
    if !base.exists() { return data; }

    for tid in TASK_IDS {
        let task_dir = base.join(tid);
        if !task_dir.exists() { continue; }
        let mut trials = Vec::new();
        let Ok(entries) = std::fs::read_dir(&task_dir) else { continue };
        let mut dirs: Vec<_> = entries.flatten().filter(|e| e.path().is_dir()).collect();
        dirs.sort_by_key(|e| e.file_name());
        for entry in dirs {
            let trial_id = entry.file_name().to_string_lossy().to_string();
            let score_path = entry.path().join("score.txt");
            let (score, detail) = if let Ok(content) = std::fs::read_to_string(&score_path) {
                let mut lines = content.lines();
                let s: f32 = lines.next().and_then(|l| l.parse().ok()).unwrap_or(-1.0);
                let d: String = lines.collect::<Vec<_>>().join("\n");
                (s, d)
            } else {
                (-1.0, String::new())
            };
            trials.push(Trial { trial_id, score, detail });
        }
        if !trials.is_empty() {
            data.insert(tid.to_string(), TaskData { trials });
        }
    }
    data
}

/// Render trial dump for log panel.
fn render_trial(task: &str, trial_idx: usize, data: &HashMap<String, TaskData>) -> String {
    let Some(td) = data.get(task) else {
        return format!("No trials for {}\nRun: cargo run -- --provider nemotron --task {}", task, task);
    };
    let idx = trial_idx.min(td.trials.len().saturating_sub(1));
    let trial = &td.trials[idx];
    let dir = format!("benchmarks/tasks/{}/{}", task, trial.trial_id);
    let mut out = String::new();

    // Header
    out.push_str(&format!("━━━ {} trial {}/{} ━━━\n", task, idx + 1, td.trials.len()));
    out.push_str(&format!("ID: {}\n", trial.trial_id));
    if trial.score >= 0.0 {
        out.push_str(&format!("Score: {:.0}\n", trial.score));
    }
    if !trial.detail.is_empty() {
        out.push_str(&format!("{}\n", trial.detail));
    }

    // BitGN URL
    if let Ok(url) = std::fs::read_to_string(format!("{}/bitgn_log.url", dir)) {
        out.push_str(&format!("\n🔗 {}\n", url.trim()));
    }

    // Pipeline
    if let Ok(c) = std::fs::read_to_string(format!("{}/pipeline.txt", dir)) {
        out.push_str("\n── Pipeline ──\n");
        out.push_str(&c);
    }

    // Inbox
    let mut inbox: Vec<_> = std::fs::read_dir(&dir).into_iter().flatten().flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("inbox_"))
        .collect();
    inbox.sort_by_key(|e| e.file_name());
    if !inbox.is_empty() {
        out.push_str("\n── Inbox ──\n");
        for f in &inbox {
            if let Ok(c) = std::fs::read_to_string(f.path()) {
                out.push_str(&c);
                out.push_str("\n---\n");
            }
        }
    }

    // Contacts
    if let Ok(c) = std::fs::read_to_string(format!("{}/contacts.txt", dir)) {
        let n = c.lines().count();
        out.push_str(&format!("\n── Contacts ({}) ──\n", n));
        for line in c.lines().take(10) { out.push_str(line); out.push('\n'); }
        if n > 10 { out.push_str("  ...\n"); }
    }

    // Accounts
    if let Ok(c) = std::fs::read_to_string(format!("{}/accounts.txt", dir)) {
        let n = c.lines().count();
        out.push_str(&format!("\n── Accounts ({}) ──\n", n));
        for line in c.lines().take(10) { out.push_str(line); out.push('\n'); }
        if n > 10 { out.push_str("  ...\n"); }
    }

    // Tree
    if let Ok(c) = std::fs::read_to_string(format!("{}/tree.txt", dir)) {
        out.push_str("\n── Tree ──\n");
        out.push_str(&c);
    }

    if out.lines().count() < 5 {
        out.push_str(&format!("\nDump dir: {}\n", dir));
    }
    out
}

fn colorize_line(line: &str) -> Style {
    if line.contains("Score: 1") || line.contains("PASS") { Style::default().fg(Color::Green).add_modifier(Modifier::BOLD) }
    else if line.contains("Score: 0") || line.contains("FAIL") { Style::default().fg(Color::Red).add_modifier(Modifier::BOLD) }
    else if line.starts_with("━") { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) }
    else if line.starts_with("──") { Style::default().fg(Color::Yellow) }
    else if line.starts_with("🔗") { Style::default().fg(Color::Blue) }
    else if line.contains("[⚠") || line.contains("⚠") { Style::default().fg(Color::Yellow) }
    else if line.contains("[✓") { Style::default().fg(Color::Green) }
    else if line.contains("CLASSIFICATION") || line.contains("sender:") { Style::default().fg(Color::Magenta) }
    else if line.starts_with("instruction:") || line.starts_with("intent:") { Style::default().fg(Color::Cyan) }
    else if line.starts_with("  -") || line.starts_with("- ") { Style::default().fg(Color::White) }
    else { Style::default().fg(Color::DarkGray) }
}

fn run_app() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut table_state = TableState::default();
    table_state.select(Some(0));
    let mut trial_select: usize = 0;
    let mut log_scroll: u16 = 0;

    let data = load_task_data();
    let max_trials = data.values().map(|d| d.trials.len()).max().unwrap_or(0);

    loop {
        let selected_task = table_state.selected().unwrap_or(0);
        let task_id = TASK_IDS[selected_task];
        let log_content = render_trial(task_id, trial_select, &data);

        terminal.draw(|f| {
            let main = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(f.area());

            // ── LEFT: Heatmap ──
            let left = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(5), Constraint::Length(1)])
                .split(main[0]);

            let tasks_with_data = data.len();
            let title = Paragraph::new(format!(
                " PAC1 — {}/{} tasks, {} trials | q ↑↓ ←→ PgUp/Dn",
                tasks_with_data, TASK_IDS.len(), max_trials
            )).style(Style::default().fg(Color::Cyan));
            f.render_widget(title, left[0]);

            // Build rows
            let rows: Vec<Row> = TASK_IDS.iter().enumerate().map(|(idx, &tid)| {
                let is_selected = idx == selected_task;
                let tid_style = if is_selected {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                let mut cells = vec![Cell::from(tid).style(tid_style)];

                if let Some(td) = data.get(tid) {
                    let mut pass = 0u32;
                    let mut total = 0u32;
                    for (ti, trial) in td.trials.iter().enumerate() {
                        let is_col_selected = is_selected && ti == trial_select.min(td.trials.len().saturating_sub(1));
                        let (ch, fg, bg) = if trial.score < 0.0 {
                            // No score yet — show as yellow "?"
                            if is_col_selected { ("?", Color::Black, Color::Yellow) }
                            else { ("?", Color::Yellow, Color::Reset) }
                        } else if trial.score >= 1.0 {
                            pass += 1; total += 1;
                            if is_col_selected { ("█", Color::Black, Color::Green) }
                            else { ("█", Color::Green, Color::Reset) }
                        } else {
                            total += 1;
                            if is_col_selected { ("█", Color::Black, Color::Red) }
                            else { ("█", Color::Red, Color::Reset) }
                        };
                        cells.push(Cell::from(ch).style(Style::default().fg(fg).bg(bg)));
                    }
                    // Rate
                    let rate = if total > 0 { pass * 100 / total } else { 0 };
                    let rc = match rate { 100 => Color::Green, 75..=99 => Color::Yellow, 50..=74 => Color::Magenta, _ => Color::Red };
                    cells.push(Cell::from(format!("{:3}%", rate)).style(Style::default().fg(rc)));
                } else {
                    cells.push(Cell::from("no data").style(Style::default().fg(Color::DarkGray)));
                }

                Row::new(cells)
            }).collect();

            // Dynamic widths: task(4) + N trial cols(1 each) + rate(5)
            let trial_cols = max_trials.min(30);
            let mut widths = vec![Constraint::Length(4)];
            for _ in 0..trial_cols { widths.push(Constraint::Length(1)); }
            widths.push(Constraint::Length(5));

            let table = Table::new(rows, widths)
                .block(Block::default().borders(Borders::RIGHT))
                .row_highlight_style(Style::default());
            f.render_stateful_widget(table, left[1], &mut table_state);

            // Status
            let n_trials = data.get(task_id).map(|d| d.trials.len()).unwrap_or(0);
            let status = Paragraph::new(format!(" {} trial {}/{}", task_id, trial_select + 1, n_trials))
                .style(Style::default().fg(Color::DarkGray));
            f.render_widget(status, left[2]);

            // ── RIGHT: Log viewer ──
            let total_lines = log_content.lines().count();
            let visible_lines = main[1].height.saturating_sub(2) as usize;
            let log_lines: Vec<Line> = log_content.lines()
                .skip(log_scroll as usize)
                .take(visible_lines)
                .map(|l| Line::from(Span::styled(l, colorize_line(l))))
                .collect();

            let log_widget = Paragraph::new(log_lines)
                .block(Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {} [{}/{}] ", task_id, log_scroll, total_lines))
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
                        trial_select += 1;
                        log_scroll = 0;
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        trial_select = trial_select.saturating_sub(1);
                        log_scroll = 0;
                    }
                    KeyCode::PageDown | KeyCode::Char('d') => {
                        let max = log_content.lines().count().saturating_sub(10) as u16;
                        log_scroll = (log_scroll + 20).min(max);
                    }
                    KeyCode::PageUp | KeyCode::Char('u') => {
                        log_scroll = log_scroll.saturating_sub(20);
                    }
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
