//! PAC1 Dashboard — TUI split view: heatmap + detail viewer.
//!
//! Data: Phoenix SQLite DB (~/.phoenix/phoenix.db), project "pac1"
//! Usage: cargo run --bin pac1-dash

use std::{io, collections::{HashMap, BTreeSet}};
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
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Terminal,
};
use rusqlite::{Connection, OpenFlags};

/// UTF-8 safe string truncation (local copy — dashboard is a separate binary).
trait StrExt {
    fn trunc(&self, max_bytes: usize) -> &str;
}
impl StrExt for str {
    #[inline]
    fn trunc(&self, max_bytes: usize) -> &str {
        &self[..self.floor_char_boundary(max_bytes)]
    }
}

const PHOENIX_DB: &str = "~/.phoenix/phoenix.db";
const PROJECT_NAME: &str = "pac1";

#[derive(Clone)]
struct Trial {
    task_id: String,
    session_id: String,
    model: String,
    score: f64,
    outcome: String,
    steps: u32,
    prompt_tokens: u64,
    completion_tokens: u64,
    start_time: String,
    llm_calls: u32,
}

struct DashData {
    trials: Vec<Trial>,
    /// task_id -> vec of trials (newest first)
    by_task: HashMap<String, Vec<Trial>>,
    /// sorted task ids
    task_ids: Vec<String>,
    /// sorted model names
    models: Vec<String>,
}

fn expand_path(p: &str) -> String {
    if p.starts_with("~/") {
        if let Some(home) = std::env::var("HOME").ok() {
            return format!("{}/{}", home, &p[2..]);
        }
    }
    p.to_string()
}

fn load_from_phoenix() -> Result<DashData, String> {
    let db_path = expand_path(PHOENIX_DB);
    let conn = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("Cannot open Phoenix DB at {}: {}", db_path, e))?;

    // Find project rowid
    let project_rowid: i64 = conn
        .query_row("SELECT id FROM projects WHERE name = ?1", [PROJECT_NAME], |row| row.get(0))
        .map_err(|_| format!("Project '{}' not found in Phoenix DB", PROJECT_NAME))?;

    // Load trial results
    let mut trial_stmt = conn.prepare(
        "SELECT JSON_EXTRACT(s.attributes, '$.task_id'),
                JSON_EXTRACT(s.attributes, '$.score'),
                JSON_EXTRACT(s.attributes, '$.outcome'),
                JSON_EXTRACT(s.attributes, '$.steps'),
                JSON_EXTRACT(s.attributes, '$.session.id'),
                s.start_time
         FROM spans s JOIN traces t ON s.trace_rowid = t.id
         WHERE t.project_rowid = ?1 AND s.name = 'trial.result'
         ORDER BY s.start_time DESC"
    ).map_err(|e| format!("SQL error: {}", e))?;

    let trial_rows: Vec<(String, f64, String, u32, String, String)> = trial_stmt
        .query_map([project_rowid], |row| {
            Ok((
                row.get::<_, String>(0).unwrap_or_default(),
                row.get::<_, f64>(1).unwrap_or(-1.0),
                row.get::<_, String>(2).unwrap_or_default(),
                row.get::<_, u32>(3).unwrap_or(0),
                row.get::<_, String>(4).unwrap_or_default(),
                row.get::<_, String>(5).unwrap_or_default(),
            ))
        })
        .map_err(|e| format!("Query error: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    // Load LLM stats grouped by session
    let mut llm_stmt = conn.prepare(
        "SELECT JSON_EXTRACT(s.attributes, '$.session.id'),
                JSON_EXTRACT(s.attributes, '$.llm.model_name'),
                SUM(COALESCE(s.llm_token_count_prompt, 0)),
                SUM(COALESCE(s.llm_token_count_completion, 0)),
                COUNT(*)
         FROM spans s JOIN traces t ON s.trace_rowid = t.id
         WHERE t.project_rowid = ?1 AND s.name IN ('chat.completions.api', 'oxide.responses.api')
         GROUP BY JSON_EXTRACT(s.attributes, '$.session.id')"
    ).map_err(|e| format!("SQL error: {}", e))?;

    let llm_stats: HashMap<String, (String, u64, u64, u32)> = llm_stmt
        .query_map([project_rowid], |row| {
            Ok((
                row.get::<_, String>(0).unwrap_or_default(),
                row.get::<_, String>(1).unwrap_or_default(),
                row.get::<_, u64>(2).unwrap_or(0),
                row.get::<_, u64>(3).unwrap_or(0),
                row.get::<_, u32>(4).unwrap_or(0),
            ))
        })
        .map_err(|e| format!("Query error: {}", e))?
        .filter_map(|r| r.ok())
        .map(|(sid, model, pt, ct, calls)| (sid, (model, pt, ct, calls)))
        .collect();

    // Merge trial rows with LLM stats
    let mut trials = Vec::new();
    let mut task_set = BTreeSet::new();
    let mut model_set = BTreeSet::new();

    for (task_id, score, outcome, steps, session_id, start_time) in &trial_rows {
        if task_id.is_empty() { continue; }
        let (model, prompt_tokens, completion_tokens, llm_calls) = llm_stats
            .get(session_id)
            .cloned()
            .unwrap_or_default();

        // Shorten model name: take last segment after '/'
        let model_short = model.rsplit('/').next().unwrap_or(&model).to_string();

        task_set.insert(task_id.clone());
        if !model_short.is_empty() {
            model_set.insert(model_short.clone());
        }

        trials.push(Trial {
            task_id: task_id.clone(),
            session_id: session_id.clone(),
            model: model_short,
            score: *score,
            outcome: outcome.clone(),
            steps: *steps,
            prompt_tokens,
            completion_tokens,
            start_time: start_time.clone(),
            llm_calls,
        });
    }

    // Build by_task map (newest first — already sorted by start_time DESC)
    let mut by_task: HashMap<String, Vec<Trial>> = HashMap::new();
    for t in &trials {
        by_task.entry(t.task_id.clone()).or_default().push(t.clone());
    }

    let task_ids: Vec<String> = task_set.into_iter().collect();
    let models: Vec<String> = if model_set.is_empty() {
        vec!["?".into()]
    } else {
        model_set.into_iter().collect()
    };

    Ok(DashData { trials, by_task, task_ids, models })
}

fn shorten_model(name: &str) -> String {
    let s = name.to_lowercase();
    if s.contains("nemotron") { "nem".into() }
    else if s.contains("seed") { "sed".into() }
    else if s.contains("gpt-5") || s.contains("gpt5") { "gpt".into() }
    else if s.contains("kimi") { "kim".into() }
    else if s.contains("minimax") || s.contains("m2.5") { "mmx".into() }
    else if name.len() > 6 { name[..4].to_string() }
    else { name.to_string() }
}

fn render_trial_detail(task: &str, model_idx: usize, data: &DashData) -> String {
    let Some(trials) = data.by_task.get(task) else {
        return format!("No trials for {}", task);
    };
    let sel_model = data.models.get(model_idx.min(data.models.len().saturating_sub(1)));
    let trial = sel_model
        .and_then(|m| trials.iter().find(|t| &t.model == m))
        .or_else(|| trials.first());
    let Some(trial) = trial else {
        return format!("No matching trial for {}", task);
    };

    let mut out = String::new();
    out.push_str(&format!("━━━ {} | {} ━━━\n", task, trial.model));
    out.push_str(&format!("Session: {}\n", trial.session_id));
    out.push_str(&format!("Time: {}\n", trial.start_time));

    let label = if trial.score >= 1.0 { "PASS" } else { "FAIL" };
    out.push_str(&format!("Score: {} ({})\n", trial.score, label));
    out.push_str(&format!("Outcome: {}\n", trial.outcome));
    out.push_str(&format!("Steps: {}\n", trial.steps));
    out.push_str(&format!("LLM calls: {}\n", trial.llm_calls));
    out.push_str(&format!("Tokens: {} prompt + {} completion = {} total\n",
        trial.prompt_tokens, trial.completion_tokens,
        trial.prompt_tokens + trial.completion_tokens));

    // Show all trials for this task+model (history)
    let same_model: Vec<&Trial> = trials.iter().filter(|t| t.model == trial.model).collect();
    if same_model.len() > 1 {
        out.push_str(&format!("\n── History ({} runs) ──\n", same_model.len()));
        for t in &same_model {
            let status = if t.score >= 1.0 { "PASS" } else { "FAIL" };
            out.push_str(&format!("  {} | {} | {}st {}llm | {}+{}tok | {}\n",
                status, t.outcome, t.steps, t.llm_calls,
                t.prompt_tokens, t.completion_tokens, &t.start_time));
        }
    }

    out
}

fn colorize(line: &str) -> Style {
    if line.contains("PASS") || line.contains("Score: 1") { Style::default().fg(Color::Green).add_modifier(Modifier::BOLD) }
    else if line.contains("FAIL") || line.contains("Score: 0") { Style::default().fg(Color::Red).add_modifier(Modifier::BOLD) }
    else if line.starts_with("━") { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) }
    else if line.starts_with("──") { Style::default().fg(Color::Yellow) }
    else if line.starts_with("Session:") || line.starts_with("Time:") { Style::default().fg(Color::DarkGray) }
    else if line.starts_with("Outcome:") { Style::default().fg(Color::Magenta) }
    else if line.starts_with("Tokens:") || line.starts_with("LLM calls:") { Style::default().fg(Color::Blue) }
    else if line.starts_with("Steps:") { Style::default().fg(Color::Cyan) }
    else { Style::default().fg(Color::White) }
}

fn run_app() -> io::Result<()> {
    // Initial load
    let mut data = match load_from_phoenix() {
        Ok(d) => d,
        Err(msg) => {
            eprintln!("Phoenix not available: {}", msg);
            eprintln!("Make sure Phoenix is running and ~/.phoenix/phoenix.db exists.");
            return Ok(());
        }
    };

    if data.task_ids.is_empty() {
        eprintln!("No trial data found in Phoenix for project '{}'.", PROJECT_NAME);
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut table_state = TableState::default();
    table_state.select(Some(0));
    let mut model_select: usize = 0;
    let mut log_scroll: usize = 0;
    let mut last_refresh = std::time::Instant::now();

    loop {
        // Auto-refresh every 5s
        if last_refresh.elapsed() >= std::time::Duration::from_secs(5) {
            if let Ok(new_data) = load_from_phoenix() {
                data = new_data;
            }
            last_refresh = std::time::Instant::now();
        }

        let sel = table_state.selected().unwrap_or(0).min(data.task_ids.len().saturating_sub(1));
        let task_id = &data.task_ids[sel];
        let log_content = render_trial_detail(task_id, model_select, &data);
        let log_total = log_content.lines().count();

        // Compute stats
        let total_pass: usize = data.by_task.values()
            .flat_map(|v| v.iter())
            .filter(|t| t.score >= 1.0)
            .count();
        let total_scored: usize = data.trials.len();
        let tasks_with_data = data.by_task.len();

        terminal.draw(|f| {
            let main = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
                .split(f.area());

            let left = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(2), Constraint::Min(5), Constraint::Length(8), Constraint::Length(2)])
                .split(main[0]);

            // Header
            let pass_rate = if total_scored > 0 { total_pass * 100 / total_scored } else { 0 };
            let header = Paragraph::new(vec![
                Line::from(vec![
                    Span::styled(" PAC1 ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::styled(format!("{}/{} tasks ", tasks_with_data, data.task_ids.len()), Style::default().fg(Color::White)),
                    Span::styled(format!("{}% pass ", pass_rate), Style::default().fg(if pass_rate > 90 { Color::Green } else { Color::Yellow })),
                    Span::styled(format!("({}/{}) ", total_pass, total_scored), Style::default().fg(Color::DarkGray)),
                    Span::styled("jk=nav hl=model Space=pgdn q=quit", Style::default().fg(Color::DarkGray)),
                ]),
            ]);
            f.render_widget(header, left[0]);

            // Heatmap table: rows=tasks, columns=models
            let model_headers: Vec<String> = data.models.iter().map(|m| shorten_model(m)).collect();

            let rows: Vec<Row> = data.task_ids.iter().enumerate().map(|(idx, tid)| {
                let is_sel = idx == sel;
                let tid_style = if is_sel {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                let mut cells = vec![Cell::from(tid.as_str()).style(tid_style)];

                if let Some(trials) = data.by_task.get(tid) {
                    for (mi, model_name) in data.models.iter().enumerate() {
                        let latest = trials.iter().find(|t| &t.model == model_name);
                        let is_col = is_sel && mi == model_select.min(data.models.len().saturating_sub(1));
                        let (ch, fg, bg) = match latest {
                            Some(t) if t.score >= 1.0 => {
                                if is_col { (" + ", Color::Black, Color::Green) }
                                else { (" + ", Color::Green, Color::Reset) }
                            }
                            Some(t) if t.score >= 0.0 => {
                                if is_col { (" x ", Color::Black, Color::Red) }
                                else { (" x ", Color::Red, Color::Reset) }
                            }
                            _ => (" . ", Color::DarkGray, Color::Reset),
                        };
                        cells.push(Cell::from(ch).style(Style::default().fg(fg).bg(bg)));
                    }
                } else {
                    for _ in &data.models {
                        cells.push(Cell::from(" . ").style(Style::default().fg(Color::DarkGray)));
                    }
                }

                Row::new(cells)
            }).collect();

            let header_cells: Vec<Cell> = std::iter::once(Cell::from("task").style(Style::default().fg(Color::DarkGray)))
                .chain(model_headers.iter().map(|h| Cell::from(h.as_str()).style(Style::default().fg(Color::Cyan))))
                .collect();
            let header_row = Row::new(header_cells).style(Style::default().add_modifier(Modifier::BOLD));

            let mut widths = vec![Constraint::Length(5)];
            for _ in &data.models { widths.push(Constraint::Length(4)); }

            let table = Table::new(rows, widths)
                .header(header_row)
                .block(Block::default().borders(Borders::RIGHT))
                .row_highlight_style(Style::default());
            f.render_stateful_widget(table, left[1], &mut table_state);

            // History panel
            let sel_model = data.models.get(model_select.min(data.models.len().saturating_sub(1))).cloned().unwrap_or_default();
            let history_lines: Vec<Line> = data.by_task.get(task_id)
                .map(|trials| trials.iter()
                    .filter(|t| t.model == sel_model)
                    .map(|t| {
                        let status = if t.score >= 1.0 { "+" } else { "x" };
                        let color = if t.score >= 1.0 { Color::Green } else { Color::Red };
                        Line::from(vec![
                            Span::styled(format!(" {} ", status), Style::default().fg(color)),
                            Span::styled(format!("{}st {}llm {}+{}tok ", t.steps, t.llm_calls, t.prompt_tokens, t.completion_tokens), Style::default().fg(Color::White)),
                            Span::styled(t.start_time.trunc(19), Style::default().fg(Color::DarkGray)),
                        ])
                    })
                    .collect())
                .unwrap_or_default();
            let history_widget = Paragraph::new(history_lines)
                .block(Block::default().borders(Borders::TOP).title(
                    Span::styled(format!(" {} history ", sel_model), Style::default().fg(Color::Cyan))
                ));
            f.render_widget(history_widget, left[2]);

            // Status bar
            let trial_info = data.by_task.get(task_id)
                .and_then(|trials| trials.iter().find(|t| t.model == sel_model))
                .map(|t| format!("{} | {} | {}st {}llm {}+{}tok",
                    t.model, t.outcome, t.steps, t.llm_calls, t.prompt_tokens, t.completion_tokens))
                .unwrap_or_else(|| format!("{} | no data", sel_model));
            let status = Paragraph::new(Line::from(vec![
                Span::styled(format!(" {} ", task_id), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(trial_info, Style::default().fg(Color::Cyan)),
            ]));
            f.render_widget(status, left[3]);

            // Right panel: detail viewer
            let all_lines: Vec<Line> = log_content.lines()
                .map(|l| Line::from(Span::styled(l, colorize(l))))
                .collect();

            let log_widget = Paragraph::new(all_lines)
                .block(Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray))
                    .title({
                        let score_info = data.by_task.get(task_id)
                            .and_then(|trials| {
                                let mi = model_select.min(data.models.len().saturating_sub(1));
                                data.models.get(mi).and_then(|m| trials.iter().find(|t| &t.model == m))
                            })
                            .map(|t| if t.score >= 1.0 { " PASS ".to_string() } else { format!(" FAIL ({}) ", t.outcome) })
                            .unwrap_or_default();
                        format!(" {}{} [{}/{}] ", task_id, score_info, log_scroll + 1, log_total)
                    })
                )
                .scroll((log_scroll as u16, 0));
            f.render_widget(log_widget, main[1]);

            // Scrollbar
            let mut sb_state = ScrollbarState::default()
                .content_length(log_total)
                .position(log_scroll);
            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .thumb_symbol("█")
                .track_symbol(Some("│"));
            f.render_stateful_widget(scrollbar, main[1], &mut sb_state);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            let ev = event::read()?;
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
                    KeyCode::Char(' ') => { log_scroll = (log_scroll + 30).min(max_scroll); }
                    KeyCode::Char('d') | KeyCode::Char('f') => { log_scroll = (log_scroll + 20).min(max_scroll); }
                    KeyCode::Char('u') | KeyCode::Char('b') => { log_scroll = log_scroll.saturating_sub(20); }
                    KeyCode::PageDown => { log_scroll = (log_scroll + 20).min(max_scroll); }
                    KeyCode::PageUp => { log_scroll = log_scroll.saturating_sub(20); }
                    KeyCode::Down if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        log_scroll = (log_scroll + 1).min(max_scroll);
                    }
                    KeyCode::Up if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        log_scroll = log_scroll.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        let i = table_state.selected().unwrap_or(0);
                        table_state.select(Some((i + 1).min(data.task_ids.len() - 1)));
                        log_scroll = 0;
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        let i = table_state.selected().unwrap_or(0);
                        table_state.select(Some(i.saturating_sub(1)));
                        log_scroll = 0;
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        model_select = (model_select + 1).min(data.models.len().saturating_sub(1));
                        log_scroll = 0;
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        model_select = model_select.saturating_sub(1);
                        log_scroll = 0;
                    }
                    KeyCode::Enter => { /* select — detail already shown */ }
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
