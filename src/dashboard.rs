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

struct Trial {
    trial_id: String,
    model: String,
    score: f32,
    steps: usize,
    tool_calls: usize,
    agent_secs: f32,
    detail: String,
}
struct TaskData { trials: Vec<Trial> }

fn parse_metrics(path: &std::path::Path) -> (String, f32, usize, usize, f32) {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let get = |key: &str| -> String {
        content.lines().find(|l| l.starts_with(key))
            .map(|l| l[key.len()..].trim().to_string())
            .unwrap_or_default()
    };
    (
        get("model: "),
        get("score: ").parse().unwrap_or(-1.0),
        get("steps: ").parse().unwrap_or(0),
        get("tool_calls: ").parse().unwrap_or(0),
        get("agent_secs: ").parse().unwrap_or(0.0),
    )
}

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
        // Sort newest first by modification time
        dirs.sort_by(|a, b| {
            let ta = a.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            let tb = b.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            tb.cmp(&ta)
        });
        for entry in dirs {
            let trial_id = entry.file_name().to_string_lossy().to_string();
            let metrics_path = entry.path().join("metrics.txt");
            let pipeline_path = entry.path().join("pipeline.txt");

            // Prefer metrics.txt, fallback to pipeline.txt, then score.txt
            let (model, score, steps, tool_calls, agent_secs, detail) = if metrics_path.exists() {
                let (m, s, st, tc, secs) = parse_metrics(&metrics_path);
                let det = std::fs::read_to_string(entry.path().join("score.txt"))
                    .ok().map(|c| c.lines().skip(1).collect::<Vec<_>>().join("\n")).unwrap_or_default();
                (m, s, st, tc, secs, det)
            } else if pipeline_path.exists() {
                let (m, s, st, tc, secs) = parse_metrics(&pipeline_path);
                (m, s, st, tc, secs, String::new())
            } else if let Ok(c) = std::fs::read_to_string(entry.path().join("score.txt")) {
                let mut l = c.lines();
                let s = l.next().and_then(|s| s.parse().ok()).unwrap_or(-1.0);
                (String::new(), s, 0, 0, 0.0, l.collect::<Vec<_>>().join("\n"))
            } else {
                continue; // skip dirs without any result files
            };

            // Skip trials with no score data
            if score < 0.0 { continue; }

            // Extract model short name from trial_id (e.g. "Seed-2.0-pro_vm-xxx" → "Seed-2.0-pro")
            let model_short = if !model.is_empty() {
                model.rsplit('/').next().unwrap_or(&model).to_string()
            } else {
                trial_id.split("_vm-").next().unwrap_or(&trial_id).to_string()
            };

            trials.push(Trial { trial_id, model: model_short, score, steps, tool_calls, agent_secs, detail });
        }
        if !trials.is_empty() { data.insert(tid.to_string(), TaskData { trials }); }
    }
    data
}

fn render_trial(task: &str, trial_idx: usize, data: &HashMap<String, TaskData>, models: &[String]) -> String {
    let Some(td) = data.get(task) else {
        return format!("No trials for {}\nRun: cargo run -- --provider nemotron --task {}", task, task);
    };
    // Find trial for selected model (column = model index)
    let sel_model = models.get(trial_idx.min(models.len().saturating_sub(1)));
    let trial = sel_model
        .and_then(|m| td.trials.iter().find(|t| &t.model == m))
        .or_else(|| td.trials.first());
    let Some(trial) = trial else {
        return format!("No matching trial for {}", task);
    };
    let dir = format!("benchmarks/tasks/{}/{}", task, trial.trial_id);
    let mut out = String::new();

    out.push_str(&format!("━━━ {} | {} ━━━\n", task, trial.model));
    out.push_str(&format!("ID: {}\n", trial.trial_id));
    if trial.score >= 0.0 {
        let label = if trial.score >= 1.0 { "PASS" } else { "FAIL" };
        out.push_str(&format!("Score: {} ({})\n", trial.score, label));
    }
    if !trial.detail.is_empty() { out.push_str(&trial.detail); out.push('\n'); }
    if let Ok(url) = std::fs::read_to_string(format!("{}/bitgn_log.url", dir)) {
        out.push_str(&format!("🔗 {}\n", url.trim()));
    }

    // Always show pipeline.txt first (classification + score + metrics)
    if let Ok(c) = std::fs::read_to_string(format!("{}/pipeline.txt", dir)) {
        out.push_str("\n── Pipeline ──\n"); out.push_str(&c); out.push('\n');
    }

    // Show metrics.txt if different from pipeline
    if let Ok(c) = std::fs::read_to_string(format!("{}/metrics.txt", dir)) {
        out.push_str("\n── Metrics ──\n"); out.push_str(&c); out.push('\n');
    }

    // Show inbox files (classifications + content)
    let mut inbox_files: Vec<_> = std::fs::read_dir(&dir).into_iter().flatten().flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("inbox_"))
        .collect();
    inbox_files.sort_by_key(|e| e.file_name());
    if !inbox_files.is_empty() {
        out.push_str(&format!("\n── Inbox ({} files) ──\n", inbox_files.len()));
        for f in &inbox_files {
            if let Ok(c) = std::fs::read_to_string(f.path()) {
                // Show first 5 lines of each inbox file
                let preview: String = c.lines().take(5).collect::<Vec<_>>().join("\n");
                out.push_str(&format!("{}\n{}\n\n", f.file_name().to_string_lossy(), preview));
            }
        }
    }

    // Agent log (run.log) — full tool call history
    if let Ok(c) = std::fs::read_to_string(format!("{}/run.log", dir)) {
        out.push_str(&format!("\n── Agent Log ({} lines) ──\n", c.lines().count()));
        out.push_str(&c);
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

    // AI-NOTE: show only finalist models (matching config.toml providers)
    // Filter by keyword match — models in dumps have full names like "nemotron-3-120b-a12b"
    let finalist_keywords = ["nemotron", "Seed", "gpt-5", "Kimi-K2"];
    let all_dump_models: Vec<String> = data.values()
        .flat_map(|td| td.trials.iter().map(|t| t.model.clone()))
        .filter(|m| !m.is_empty())
        .collect();

    let mut models: Vec<String> = Vec::new();
    for kw in &finalist_keywords {
        if let Some(m) = all_dump_models.iter().find(|m| m.contains(kw)) {
            if !models.contains(m) { models.push(m.clone()); }
        }
    }
    if models.is_empty() {
        // Fallback: top 4 by frequency
        let mut counts: HashMap<String, usize> = HashMap::new();
        for m in &all_dump_models { *counts.entry(m.clone()).or_default() += 1; }
        let mut sorted: Vec<_> = counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        models = sorted.into_iter().take(4).map(|(k, _)| k).collect();
    }
    if models.is_empty() { models.push("?".into()); }

    let max_trials = data.values().map(|d| d.trials.len()).max().unwrap_or(0);
    let tasks_with_data = data.len();
    let total_pass: usize = data.values().flat_map(|d| &d.trials).filter(|t| t.score >= 1.0).count();
    let total_scored: usize = data.values().flat_map(|d| &d.trials).filter(|t| t.score >= 0.0).count();

    loop {
        let sel = table_state.selected().unwrap_or(0);
        let task_id = TASK_IDS[sel];
        let log_content = render_trial(task_id, trial_select, &data, &models);
        let log_total = log_content.lines().count();

        terminal.draw(|f| {
            let main = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
                .split(f.area());

            let left = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(2), Constraint::Min(5), Constraint::Length(8), Constraint::Length(2)])
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

            // ── Heatmap table: rows=tasks, columns=models (latest trial per model) ──
            let model_headers: Vec<String> = models.iter()
                .map(|m| {
                    // Shorten model name for column header: "nemotron-3-120b-a12b" → "nem"
                    let s = m.as_str();
                    if s.contains("nemotron") { "nem".into() }
                    else if s.contains("Seed") { "sed".into() }
                    else if s.contains("gpt-5") { "gpt".into() }
                    else if s.contains("Kimi") { "kim".into() }
                    else if s.len() > 4 { s[..3].to_string() }
                    else { s.to_string() }
                })
                .collect();

            let rows: Vec<Row> = TASK_IDS.iter().enumerate().map(|(idx, &tid)| {
                let is_sel = idx == sel;
                let tid_style = if is_sel {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                let mut cells = vec![Cell::from(tid).style(tid_style)];

                if let Some(td) = data.get(tid) {
                    // One cell per model: latest trial for that model
                    for (mi, model_name) in models.iter().enumerate() {
                        let latest = td.trials.iter().find(|t| &t.model == model_name);
                        let is_col = is_sel && mi == trial_select.min(models.len().saturating_sub(1));
                        let (ch, fg, bg) = match latest {
                            Some(t) if t.score >= 1.0 => {
                                if is_col { (" ✓ ", Color::Black, Color::Green) }
                                else { (" ✓ ", Color::Green, Color::Reset) }
                            }
                            Some(t) if t.score >= 0.0 => {
                                if is_col { (" ✗ ", Color::Black, Color::Red) }
                                else { (" ✗ ", Color::Red, Color::Reset) }
                            }
                            _ => {
                                (" · ", Color::DarkGray, Color::Reset)
                            }
                        };
                        cells.push(Cell::from(ch).style(Style::default().fg(fg).bg(bg)));
                    }
                }

                Row::new(cells)
            }).collect();

            // Header row with model short names
            let header_cells: Vec<Cell> = std::iter::once(Cell::from("task").style(Style::default().fg(Color::DarkGray)))
                .chain(model_headers.iter().map(|h| Cell::from(h.as_str()).style(Style::default().fg(Color::Cyan))))
                .collect();
            let header_row = Row::new(header_cells).style(Style::default().add_modifier(Modifier::BOLD));

            let mut widths = vec![Constraint::Length(4)];
            for _ in &models { widths.push(Constraint::Length(3)); }

            let table = Table::new(rows, widths)
                .header(header_row)
                .block(Block::default().borders(Borders::RIGHT))
                .row_highlight_style(Style::default());
            f.render_stateful_widget(table, left[1], &mut table_state);

            // ── History panel: all runs of selected model on selected task ──
            let sel_model = models.get(trial_select.min(models.len().saturating_sub(1))).cloned().unwrap_or_default();
            let history_lines: Vec<Line> = data.get(task_id)
                .map(|td| td.trials.iter()
                    .filter(|t| t.model == sel_model)
                    .map(|t| {
                        let status = if t.score >= 1.0 { "✓" } else { "✗" };
                        let color = if t.score >= 1.0 { Color::Green } else { Color::Red };
                        Line::from(vec![
                            Span::styled(format!(" {} ", status), Style::default().fg(color)),
                            Span::styled(format!("{:.0}s {}st {}tc ", t.agent_secs, t.steps, t.tool_calls), Style::default().fg(Color::White)),
                            Span::styled(&t.trial_id[..t.trial_id.len().min(20)], Style::default().fg(Color::DarkGray)),
                        ])
                    })
                    .collect())
                .unwrap_or_default();
            let history_widget = Paragraph::new(history_lines)
                .block(Block::default().borders(Borders::TOP).title(
                    Span::styled(format!(" {} history ", sel_model), Style::default().fg(Color::Cyan))
                ));
            f.render_widget(history_widget, left[2]);

            // ── Status bar ──
            let trial_info = data.get(task_id)
                .and_then(|td| td.trials.iter().find(|t| t.model == sel_model))
                .map(|t| format!("{} | {:.2} {:.0}s {}steps {}tools",
                    t.model, t.score, t.agent_secs, t.steps, t.tool_calls))
                .unwrap_or_else(|| format!("{} | no data", sel_model));
            let status = Paragraph::new(Line::from(vec![
                Span::styled(format!(" {} ", task_id), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(trial_info, Style::default().fg(Color::Cyan)),
            ]));
            f.render_widget(status, left[3]);

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
