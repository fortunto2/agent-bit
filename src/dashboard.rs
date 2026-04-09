//! PAC1 Dashboard — TUI split view: heatmap + log viewer.
//!
//! Data: benchmarks/tasks/{task_id}/{trial_id}/score.txt + dump files
//! Usage: cargo run --bin pac1-dash

use std::{io, collections::HashMap};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers, MouseEventKind, EnableMouseCapture, DisableMouseCapture},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    execute,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Terminal,
};

const TASK_IDS: &[&str] = &[
    "t01","t02","t03","t04","t05","t06","t07","t08","t09","t10",
    "t11","t12","t13","t14","t15","t16","t17","t18","t19","t20",
    "t21","t22","t23","t24","t25","t26","t27","t28","t29","t30",
    "t31","t32","t33","t34","t35","t36","t37","t38","t39","t40",
    "t41","t42","t43",
];

struct Trial { trial_id: String, score: f32, detail: String }
struct TaskData { trials: Vec<Trial> }

fn load_task_data() -> HashMap<String, TaskData> {
    let mut data: HashMap<String, TaskData> = HashMap::new();
    let base = std::path::Path::new("benchmarks/tasks");
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
            let (score, detail) = if let Ok(c) = std::fs::read_to_string(entry.path().join("score.txt")) {
                let mut l = c.lines();
                (l.next().and_then(|s| s.parse().ok()).unwrap_or(-1.0), l.collect::<Vec<_>>().join("\n"))
            } else { (-1.0, String::new()) };
            trials.push(Trial { trial_id, score, detail });
        }
        if !trials.is_empty() { data.insert(tid.to_string(), TaskData { trials }); }
    }
    data
}

fn render_trial(task: &str, trial_idx: usize, data: &HashMap<String, TaskData>) -> String {
    let Some(td) = data.get(task) else {
        return format!("No trials for {}\nRun: cargo run -- --provider nemotron --task {}", task, task);
    };
    let idx = trial_idx.min(td.trials.len().saturating_sub(1));
    let trial = &td.trials[idx];
    let dir = format!("benchmarks/tasks/{}/{}", task, trial.trial_id);
    let mut out = String::new();

    out.push_str(&format!("━━━ {} trial {}/{} ━━━\n", task, idx + 1, td.trials.len()));
    out.push_str(&format!("ID: {}\n", trial.trial_id));
    if trial.score >= 0.0 {
        let label = if trial.score >= 1.0 { "PASS" } else { "FAIL" };
        out.push_str(&format!("Score: {} ({})\n", trial.score, label));
    }
    if !trial.detail.is_empty() { out.push_str(&trial.detail); out.push('\n'); }
    if let Ok(url) = std::fs::read_to_string(format!("{}/bitgn_log.url", dir)) {
        out.push_str(&format!("🔗 {}\n", url.trim()));
    }

    // Full agent log (run.log) — primary view if available
    if let Ok(c) = std::fs::read_to_string(format!("{}/run.log", dir)) {
        out.push_str(&format!("\n── Agent Log ({} lines) ──\n", c.lines().count()));
        out.push_str(&c);
        return out; // run.log is complete — skip dump file rendering
    }

    // Fallback: render dump files
    if let Ok(c) = std::fs::read_to_string(format!("{}/pipeline.txt", dir)) {
        out.push_str("\n── Pipeline ──\n"); out.push_str(&c); out.push('\n');
    }
    // All dump files
    let mut files: Vec<_> = std::fs::read_dir(&dir).into_iter().flatten().flatten()
        .filter(|e| e.path().is_file() && e.file_name().to_string_lossy() != "score.txt"
            && e.file_name().to_string_lossy() != "bitgn_log.url"
            && e.file_name().to_string_lossy() != "pipeline.txt")
        .collect();
    files.sort_by_key(|e| e.file_name());
    for f in &files {
        let name = f.file_name().to_string_lossy().to_string();
        if let Ok(c) = std::fs::read_to_string(f.path()) {
            let preview_lines = if name.contains("inbox") { 20 } else { 15 };
            let lines: Vec<&str> = c.lines().take(preview_lines).collect();
            let total = c.lines().count();
            out.push_str(&format!("\n── {} ({} lines) ──\n", name, total));
            out.push_str(&lines.join("\n"));
            if total > preview_lines { out.push_str("\n  ..."); }
            out.push('\n');
        }
    }
    out
}

fn colorize(line: &str) -> Style {
    if line.contains("PASS") || line.contains("Score: 1") { Style::default().fg(Color::Green).add_modifier(Modifier::BOLD) }
    else if line.contains("FAIL") || line.contains("Score: 0") { Style::default().fg(Color::Red).add_modifier(Modifier::BOLD) }
    else if line.starts_with("━") { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) }
    else if line.starts_with("──") || line.starts_with("── ") { Style::default().fg(Color::Yellow) }
    else if line.starts_with("🔗") { Style::default().fg(Color::Blue) }
    else if line.contains("[⚠") || line.contains("⚠") { Style::default().fg(Color::Yellow) }
    else if line.contains("[✓") || line.contains("TRUSTED") { Style::default().fg(Color::Green) }
    else if line.contains("CLASSIFICATION") || line.contains("sender:") || line.contains("injection") || line.contains("credential") { Style::default().fg(Color::Magenta) }
    else if line.starts_with("instruction:") || line.starts_with("intent:") || line.starts_with("label:") { Style::default().fg(Color::Cyan) }
    else if line.starts_with("$ cat") || line.starts_with("$ ls") || line.starts_with("$ tree") { Style::default().fg(Color::Blue) }
    else if line.starts_with("ID:") || line.starts_with("inbox_files:") { Style::default().fg(Color::DarkGray) }
    else { Style::default().fg(Color::White) }
}

fn run_app() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut table_state = TableState::default();
    table_state.select(Some(0));
    let mut trial_select: usize = 0;
    let mut log_scroll: usize = 0;

    let data = load_task_data();
    let max_trials = data.values().map(|d| d.trials.len()).max().unwrap_or(0);

    // Compute stats
    let tasks_with_data = data.len();
    let _total_trials: usize = data.values().map(|d| d.trials.len()).sum();
    let total_pass: usize = data.values().flat_map(|d| &d.trials).filter(|t| t.score >= 1.0).count();
    let total_scored: usize = data.values().flat_map(|d| &d.trials).filter(|t| t.score >= 0.0).count();

    loop {
        let sel = table_state.selected().unwrap_or(0);
        let task_id = TASK_IDS[sel];
        let log_content = render_trial(task_id, trial_select, &data);
        let log_total = log_content.lines().count();

        terminal.draw(|f| {
            let main = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
                .split(f.area());

            let left = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(2), Constraint::Min(5), Constraint::Length(2)])
                .split(main[0]);

            // ── Header with metrics ──
            let pass_rate = if total_scored > 0 { total_pass * 100 / total_scored } else { 0 };
            let header = Paragraph::new(vec![
                Line::from(vec![
                    Span::styled(" PAC1 ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::styled(format!("{}/{} tasks ", tasks_with_data, TASK_IDS.len()), Style::default().fg(Color::White)),
                    Span::styled(format!("{}% pass ", pass_rate), Style::default().fg(if pass_rate > 90 { Color::Green } else { Color::Yellow })),
                    Span::styled(format!("({}/{}) ", total_pass, total_scored), Style::default().fg(Color::DarkGray)),
                    Span::styled("↑↓←→ Space o=open q=quit", Style::default().fg(Color::DarkGray)),
                ]),
            ]);
            f.render_widget(header, left[0]);

            // ── Heatmap table ──
            let rows: Vec<Row> = TASK_IDS.iter().enumerate().map(|(idx, &tid)| {
                let is_sel = idx == sel;
                let tid_style = if is_sel {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                let mut cells = vec![Cell::from(tid).style(tid_style)];

                if let Some(td) = data.get(tid) {
                    let mut pass = 0u32;
                    let mut scored = 0u32;
                    for (ti, trial) in td.trials.iter().enumerate() {
                        let is_col = is_sel && ti == trial_select.min(td.trials.len().saturating_sub(1));
                        let has_log = std::path::Path::new(&format!(
                            "benchmarks/tasks/{}/{}/run.log", tid, trial.trial_id
                        )).exists() || std::path::Path::new(&format!(
                            "benchmarks/tasks/{}/{}/pipeline.txt", tid, trial.trial_id
                        )).exists();

                        let (ch, fg, bg) = if trial.score < 0.0 {
                            if is_col { ("?", Color::Black, Color::Yellow) }
                            else { ("?", Color::DarkGray, Color::Reset) }
                        } else if trial.score >= 1.0 {
                            pass += 1; scored += 1;
                            // Dim green (▓) for score-only, bright green (█) for full data
                            if is_col { ("█", Color::Black, Color::Green) }
                            else if has_log { ("█", Color::Green, Color::Reset) }
                            else { ("▓", Color::DarkGray, Color::Green) }
                        } else {
                            scored += 1;
                            if is_col { ("█", Color::Black, Color::Red) }
                            else if has_log { ("█", Color::Red, Color::Reset) }
                            else { ("▓", Color::DarkGray, Color::Red) }
                        };
                        cells.push(Cell::from(ch).style(Style::default().fg(fg).bg(bg)));
                    }
                    // Show rate only when enough data (3+ trials)
                    if scored >= 3 {
                        let rate = pass * 100 / scored;
                        let rc = match rate { 100 => Color::Green, 75..=99 => Color::Yellow, 50..=74 => Color::Magenta, _ => Color::Red };
                        cells.push(Cell::from(format!("{:3}%", rate)).style(Style::default().fg(rc)));
                    }
                }
                // no data → empty row, just task id

                Row::new(cells)
            }).collect();

            let mut widths = vec![Constraint::Length(4)];
            for _ in 0..max_trials.min(30) { widths.push(Constraint::Length(1)); }
            widths.push(Constraint::Length(5));

            let table = Table::new(rows, widths)
                .block(Block::default().borders(Borders::RIGHT))
                .row_highlight_style(Style::default());
            f.render_stateful_widget(table, left[1], &mut table_state);

            // ── Status bar ──
            let n_trials = data.get(task_id).map(|d| d.trials.len()).unwrap_or(0);
            let status = Paragraph::new(Line::from(vec![
                Span::styled(format!(" {} ", task_id), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(format!("trial {}/{} ", trial_select.min(n_trials) + 1, n_trials), Style::default().fg(Color::White)),
            ]));
            f.render_widget(status, left[2]);

            // ── RIGHT: Log viewer ──
            // Build ALL lines (not sliced) — let Paragraph.scroll() handle offset
            let all_lines: Vec<Line> = log_content.lines()
                .map(|l| Line::from(Span::styled(l, colorize(l))))
                .collect();

            let log_widget = Paragraph::new(all_lines)
                .block(Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray))
                    .title({
                        let score_info = data.get(task_id)
                            .and_then(|td| td.trials.get(trial_select.min(td.trials.len().saturating_sub(1))))
                            .map(|t| if t.score >= 1.0 { " PASS ".to_string() } else if t.score >= 0.0 { format!(" FAIL ({}) ", t.detail.lines().next().unwrap_or("")) } else { " ? ".to_string() })
                            .unwrap_or_default();
                        format!(" {}{} [{}/{}] ", task_id, score_info, log_scroll + 1, log_total)
                    })
                )
                .scroll((log_scroll as u16, 0));
            f.render_widget(log_widget, main[1]);

            // Scrollbar
            use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
            let mut sb_state = ScrollbarState::default()
                .content_length(log_total)
                .position(log_scroll);
            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .thumb_symbol("█")
                .track_symbol(Some("│"));
            f.render_stateful_widget(scrollbar, main[1], &mut sb_state);
        })?;

        if event::poll(std::time::Duration::from_millis(50))? {
            let ev = event::read()?;
            // Mouse scroll → log scroll
            if let Event::Mouse(mouse) = &ev {
                let max_scroll = log_total.saturating_sub(5);
                match mouse.kind {
                    MouseEventKind::ScrollDown => { log_scroll = (log_scroll + 3).min(max_scroll); }
                    MouseEventKind::ScrollUp => { log_scroll = log_scroll.saturating_sub(3); }
                    _ => {}
                }
            }
            if let Event::Key(key) = ev {
                let max_scroll = log_total.saturating_sub(5);
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    // o = open BitGN URL in browser
                    KeyCode::Char('o') => {
                        if let Some(td) = data.get(task_id) {
                            let idx = trial_select.min(td.trials.len().saturating_sub(1));
                            let dir = format!("benchmarks/tasks/{}/{}", task_id, td.trials[idx].trial_id);
                            if let Ok(url) = std::fs::read_to_string(format!("{}/bitgn_log.url", dir)) {
                                let _ = std::process::Command::new("open").arg(url.trim()).spawn();
                            }
                        }
                    }
                    // Space = scroll down one page
                    KeyCode::Char(' ') => { log_scroll = (log_scroll + 30).min(max_scroll); }
                    // d/u ALWAYS scroll log
                    KeyCode::Char('d') | KeyCode::Char('f') => { log_scroll = (log_scroll + 20).min(max_scroll); }
                    KeyCode::Char('u') | KeyCode::Char('b') => { log_scroll = log_scroll.saturating_sub(20); }
                    KeyCode::PageDown => { log_scroll = (log_scroll + 20).min(max_scroll); }
                    KeyCode::PageUp => { log_scroll = log_scroll.saturating_sub(20); }
                    // Ctrl+Down/Up = scroll log by 1 line
                    KeyCode::Down if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        log_scroll = (log_scroll + 1).min(max_scroll);
                    }
                    KeyCode::Up if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        log_scroll = log_scroll.saturating_sub(1);
                    }
                    // ↑↓ = navigate tasks
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
                    // ←→ = navigate trials
                    KeyCode::Right | KeyCode::Char('l') => {
                        trial_select += 1; log_scroll = 0;
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        trial_select = trial_select.saturating_sub(1); log_scroll = 0;
                    }
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}

fn main() {
    if let Err(e) = run_app() { eprintln!("Error: {}", e); }
}
