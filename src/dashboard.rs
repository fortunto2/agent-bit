//! PAC1 Dashboard — TUI heatmap of benchmark runs.
//!
//! Reads LOG.md benchmark history + benchmarks/runs/ for task-level results.
//! Shows: task matrix (43 tasks × N runs), pass/fail heatmap, stability stats.
//!
//! Usage: cargo run --bin pac1-dash

use std::io;
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    execute,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Terminal,
};

/// A benchmark run parsed from LOG.md.
struct Run {
    date: String,
    commit: String,
    score: String,
    failures: Vec<String>,
}

/// Task stability info.
struct TaskInfo {
    id: String,
    hint: String,
    status: String,
    results: Vec<bool>, // per-run pass/fail
}

fn parse_log_md() -> (Vec<Run>, Vec<TaskInfo>) {
    let content = std::fs::read_to_string("LOG.md").unwrap_or_default();
    let mut runs = Vec::new();
    let mut tasks = Vec::new();
    let mut in_benchmark = false;
    let mut in_stability = false;

    for line in content.lines() {
        // Benchmark History table
        if line.contains("| Date | Commit |") {
            in_benchmark = true;
            in_stability = false;
            continue;
        }
        if in_benchmark && line.starts_with("| ") && !line.contains("---") {
            let cols: Vec<&str> = line.split('|').map(|s| s.trim()).collect();
            if cols.len() >= 6 {
                let failures: Vec<String> = cols[5].split(", ")
                    .filter(|s| s.starts_with('t'))
                    .map(|s| s.split_whitespace().next().unwrap_or(s).to_string())
                    .collect();
                runs.push(Run {
                    date: cols[1].to_string(),
                    commit: cols[2].trim_matches('`').to_string(),
                    score: cols[4].to_string(),
                    failures,
                });
            }
        }
        if in_benchmark && line.starts_with("---") && !line.contains("|") {
            in_benchmark = false;
        }

        // Task Stability Matrix
        if line.contains("| Task | Hint |") {
            in_stability = true;
            in_benchmark = false;
            continue;
        }
        if in_stability && line.starts_with("| t") {
            let cols: Vec<&str> = line.split('|').map(|s| s.trim()).collect();
            if cols.len() >= 6 {
                tasks.push(TaskInfo {
                    id: cols[1].to_string(),
                    hint: cols[2].to_string(),
                    status: cols.last().unwrap_or(&"").to_string(),
                    results: Vec::new(),
                });
            }
        }
        if in_stability && line.is_empty() {
            in_stability = false;
        }
    }

    // Cross-reference: for each task, check which runs it failed in
    for task in &mut tasks {
        for run in &runs {
            let failed = run.failures.iter().any(|f| f == &task.id);
            task.results.push(!failed);
        }
    }

    (runs, tasks)
}

fn run_app() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut table_state = TableState::default();
    table_state.select(Some(0));

    let (runs, tasks) = parse_log_md();

    // Show last N runs (fit screen)
    let max_runs = 8;
    let recent_runs: Vec<&Run> = runs.iter().rev().take(max_runs).collect::<Vec<_>>().into_iter().rev().collect();

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),  // header
                    Constraint::Min(10),    // heatmap
                    Constraint::Length(5),  // summary
                ])
                .split(f.area());

            // Header
            let header_text = format!(
                " PAC1 Dashboard — {} tasks × {} runs | Best: {} | q=quit ↑↓=scroll",
                tasks.len(), runs.len(),
                runs.last().map(|r| r.score.as_str()).unwrap_or("?")
            );
            let header = Paragraph::new(header_text)
                .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                .block(Block::default().borders(Borders::BOTTOM));
            f.render_widget(header, chunks[0]);

            // Heatmap table
            let mut header_cells = vec![
                Cell::from("Task").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Cell::from("Hint").style(Style::default().fg(Color::DarkGray)),
            ];
            for run in &recent_runs {
                header_cells.push(
                    Cell::from(format!("{}", &run.date))
                        .style(Style::default().fg(Color::DarkGray))
                );
            }
            header_cells.push(Cell::from("Status").style(Style::default().fg(Color::Yellow)));

            let header_row = Row::new(header_cells).height(1);

            let rows: Vec<Row> = tasks.iter().map(|task| {
                let mut cells = vec![
                    Cell::from(task.id.clone()).style(Style::default().fg(Color::White)),
                    Cell::from(if task.hint.len() > 25 {
                        format!("{}…", &task.hint[..24])
                    } else {
                        task.hint.clone()
                    }).style(Style::default().fg(Color::DarkGray)),
                ];

                // Heatmap cells for recent runs
                let start = if task.results.len() > max_runs { task.results.len() - max_runs } else { 0 };
                for i in start..task.results.len() {
                    let (symbol, color) = if task.results[i] {
                        ("✓", Color::Green)
                    } else {
                        ("✗", Color::Red)
                    };
                    cells.push(Cell::from(symbol).style(Style::default().fg(color)));
                }
                // Pad if fewer results than runs
                for _ in task.results.len()..recent_runs.len() {
                    cells.push(Cell::from("·").style(Style::default().fg(Color::DarkGray)));
                }

                // Status
                let status_color = if task.status.contains("stable") { Color::Green }
                    else if task.status.contains("fixed") || task.status.contains("FIXED") { Color::Cyan }
                    else if task.status.contains("improved") { Color::Yellow }
                    else if task.status.contains("non-det") { Color::Magenta }
                    else if task.status.contains("PERSISTENT") { Color::Red }
                    else { Color::DarkGray };
                cells.push(Cell::from(task.status.clone()).style(Style::default().fg(status_color)));

                Row::new(cells)
            }).collect();

            let widths = {
                let mut w = vec![Constraint::Length(4), Constraint::Length(26)];
                for _ in &recent_runs {
                    w.push(Constraint::Length(6));
                }
                w.push(Constraint::Min(20));
                w
            };

            let table = Table::new(rows, widths)
                .header(header_row)
                .block(Block::default().borders(Borders::ALL).title(" Task Heatmap "))
                .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));
            f.render_stateful_widget(table, chunks[1], &mut table_state);

            // Summary
            let stable = tasks.iter().filter(|t| t.status.contains("stable")).count();
            let fixed = tasks.iter().filter(|t| t.status.contains("fixed") || t.status.contains("FIXED")).count();
            let nondet = tasks.iter().filter(|t| t.status.contains("non-det")).count();
            let persistent = tasks.iter().filter(|t| t.status.contains("PERSISTENT")).count();

            let summary = Paragraph::new(vec![
                Line::from(vec![
                    Span::styled(format!(" Stable: {} ", stable), Style::default().fg(Color::Green)),
                    Span::styled(format!(" Fixed: {} ", fixed), Style::default().fg(Color::Cyan)),
                    Span::styled(format!(" Non-det: {} ", nondet), Style::default().fg(Color::Magenta)),
                    Span::styled(format!(" Persistent: {} ", persistent), Style::default().fg(Color::Red)),
                    Span::styled(format!(" Total: {}/43", stable + fixed), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                ]),
            ]).block(Block::default().borders(Borders::TOP).title(" Summary "));
            f.render_widget(summary, chunks[2]);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Down | KeyCode::Char('j') => {
                        let i = table_state.selected().unwrap_or(0);
                        table_state.select(Some((i + 1).min(tasks.len().saturating_sub(1))));
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        let i = table_state.selected().unwrap_or(0);
                        table_state.select(Some(i.saturating_sub(1)));
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
