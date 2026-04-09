//! PAC1 Dashboard — TUI heatmap of benchmark runs.
//!
//! Data sources:
//! 1. benchmarks/runs/*.md — per-task results from each run
//! 2. LOG.md — benchmark history summary
//!
//! Usage: cargo run --bin pac1-dash

use std::{io, path::Path};
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

struct RunData {
    label: String,  // "04-01 nemotron"
    score: String,  // "50% (15/30)"
    tasks: std::collections::HashMap<String, f32>,  // task_id → score
}

fn parse_run_file(path: &Path) -> Option<RunData> {
    let content = std::fs::read_to_string(path).ok()?;
    let filename = path.file_stem()?.to_str()?;
    // Format: 2026-04-01__nemotron__3cf84f2
    let parts: Vec<&str> = filename.split("__").collect();
    let date = parts.first().map(|d| &d[5..]).unwrap_or("?"); // strip year
    let provider = parts.get(1).unwrap_or(&"?");
    let label = format!("{} {}", date, provider);

    let mut score_line = String::new();
    let mut tasks = std::collections::HashMap::new();

    for line in content.lines() {
        if line.starts_with("**Score:**") {
            score_line = line.replace("**Score:**", "").trim().to_string();
        }
        if line.starts_with("| t") {
            let cols: Vec<&str> = line.split('|').map(|s| s.trim()).collect();
            if cols.len() >= 3 {
                let task_id = cols[1].to_string();
                let score: f32 = cols[2].parse().unwrap_or(-1.0);
                tasks.insert(task_id, score);
            }
        }
    }

    Some(RunData { label, score: score_line, tasks })
}

fn parse_log_md_runs() -> Vec<RunData> {
    let content = std::fs::read_to_string("LOG.md").unwrap_or_default();
    let mut runs = Vec::new();

    for line in content.lines() {
        if line.starts_with("| ") && line.contains('`') && line.contains('%') {
            let cols: Vec<&str> = line.split('|').map(|s| s.trim()).collect();
            if cols.len() >= 6 {
                let date = cols[1];
                let score = cols[4];
                let failures_str = cols[5];
                let failures: Vec<&str> = failures_str.split(", ")
                    .filter(|s| s.starts_with('t'))
                    .map(|s| s.split_whitespace().next().unwrap_or(s))
                    .collect();

                let mut tasks = std::collections::HashMap::new();
                for tid in TASK_IDS {
                    if failures.contains(tid) {
                        tasks.insert(tid.to_string(), 0.0);
                    } else {
                        tasks.insert(tid.to_string(), 1.0);
                    }
                }

                runs.push(RunData {
                    label: format!("{} {}", date, &score[..score.len().min(12)]),
                    score: score.to_string(),
                    tasks,
                });
            }
        }
    }
    runs
}

fn load_all_runs() -> Vec<RunData> {
    // Try benchmarks/runs/ first
    let runs_dir = Path::new("benchmarks/runs");
    let mut runs: Vec<RunData> = if runs_dir.exists() {
        let mut entries: Vec<_> = std::fs::read_dir(runs_dir)
            .into_iter().flatten().flatten()
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
            .collect();
        entries.sort_by_key(|e| e.file_name());
        entries.iter().filter_map(|e| parse_run_file(&e.path())).collect()
    } else {
        Vec::new()
    };

    // Supplement with LOG.md benchmark history
    let log_runs = parse_log_md_runs();
    if runs.is_empty() {
        runs = log_runs;
    } else {
        // Append LOG.md runs that aren't in benchmarks/runs/
        for lr in log_runs {
            if !runs.iter().any(|r| r.score == lr.score) {
                runs.push(lr);
            }
        }
    }
    runs
}

fn run_app() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut table_state = TableState::default();
    table_state.select(Some(0));
    let mut scroll_x: usize = 0;

    let runs = load_all_runs();
    let max_visible_runs = 10;

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(10),
                    Constraint::Length(3),
                ])
                .split(f.area());

            // Header
            let latest_score = runs.last().map(|r| r.score.as_str()).unwrap_or("?");
            let header = Paragraph::new(format!(
                " PAC1 Heatmap — {} runs | Latest: {} | q=quit ↑↓=scroll ←→=pan runs",
                runs.len(), latest_score
            ))
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .block(Block::default().borders(Borders::BOTTOM));
            f.render_widget(header, chunks[0]);

            // Visible runs window
            let end = runs.len().min(scroll_x + max_visible_runs);
            let start = end.saturating_sub(max_visible_runs);
            let visible: Vec<&RunData> = runs[start..end].iter().collect();

            // Header row
            let mut hdr_cells = vec![
                Cell::from("Task").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ];
            for run in &visible {
                let label = if run.label.len() > 14 { &run.label[..14] } else { &run.label };
                hdr_cells.push(Cell::from(label.to_string()).style(Style::default().fg(Color::DarkGray)));
            }
            hdr_cells.push(Cell::from("Rate").style(Style::default().fg(Color::Yellow)));

            // Task rows
            let rows: Vec<Row> = TASK_IDS.iter().map(|&tid| {
                let mut cells = vec![
                    Cell::from(tid).style(Style::default().fg(Color::White)),
                ];

                let mut pass_count = 0;
                let mut total_count = 0;

                for run in &visible {
                    let score = run.tasks.get(tid).copied().unwrap_or(-1.0);
                    let (block, color) = if score < 0.0 {
                        ("·", Color::DarkGray) // no data
                    } else if score >= 1.0 {
                        pass_count += 1;
                        total_count += 1;
                        ("██", Color::Green)
                    } else {
                        total_count += 1;
                        ("██", Color::Red)
                    };
                    cells.push(Cell::from(block).style(Style::default().fg(color)));
                }

                // Pass rate
                let rate = if total_count > 0 {
                    let pct = pass_count * 100 / total_count;
                    let color = if pct == 100 { Color::Green }
                        else if pct >= 75 { Color::Yellow }
                        else if pct >= 50 { Color::Magenta }
                        else { Color::Red };
                    Cell::from(format!("{:3}%", pct)).style(Style::default().fg(color))
                } else {
                    Cell::from("  -").style(Style::default().fg(Color::DarkGray))
                };
                cells.push(rate);

                Row::new(cells)
            }).collect();

            let mut widths = vec![Constraint::Length(4)];
            for _ in &visible { widths.push(Constraint::Length(15)); }
            widths.push(Constraint::Length(5));

            let table = Table::new(rows, widths)
                .header(Row::new(hdr_cells).height(1))
                .block(Block::default().borders(Borders::ALL).title(" ██ Pass  ██ Fail  · No data "))
                .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));
            f.render_stateful_widget(table, chunks[1], &mut table_state);

            // Summary line
            let total_tasks = TASK_IDS.len();
            let always_pass = TASK_IDS.iter().filter(|&&tid| {
                visible.iter().all(|r| r.tasks.get(tid).copied().unwrap_or(0.0) >= 1.0)
            }).count();
            let summary = Paragraph::new(Line::from(vec![
                Span::styled(format!(" Always pass: {}/{} ", always_pass, total_tasks),
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::styled(format!("| Runs {}-{} of {} ", start+1, end, runs.len()),
                    Style::default().fg(Color::DarkGray)),
            ])).block(Block::default().borders(Borders::TOP));
            f.render_widget(summary, chunks[2]);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Down | KeyCode::Char('j') => {
                        let i = table_state.selected().unwrap_or(0);
                        table_state.select(Some((i + 1).min(TASK_IDS.len() - 1)));
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        let i = table_state.selected().unwrap_or(0);
                        table_state.select(Some(i.saturating_sub(1)));
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        if scroll_x + max_visible_runs < runs.len() { scroll_x += 1; }
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        scroll_x = scroll_x.saturating_sub(1);
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
