use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Row, Table, Wrap},
    Frame, Terminal,
};
use std::collections::VecDeque;
use std::io::{self, Stdout};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use crate::config::DashboardConfig;
use crate::event::{RequestEvent, RequestInfo};

pub struct Dashboard {
    config: DashboardConfig,
    total_tokens: u64,
    requests: VecDeque<RequestInfo>,
    last_prompt: String,
    last_provider: String,
}

impl Dashboard {
    pub fn new(config: DashboardConfig) -> Self {
        Self {
            config,
            total_tokens: 0,
            requests: VecDeque::new(),
            last_prompt: String::new(),
            last_provider: String::new(),
        }
    }

    pub async fn run(
        mut self,
        mut event_rx: mpsc::Receiver<RequestEvent>,
        archive_tx: mpsc::Sender<RequestEvent>,
    ) -> Result<()> {
        let mut terminal = setup_terminal()?;

        let tick_rate = Duration::from_millis(1000 / self.config.refresh_rate_hz as u64);
        let mut last_tick = Instant::now();

        loop {
            // Draw UI
            terminal.draw(|f| self.render(f))?;

            // Handle events with timeout
            let timeout = tick_rate.saturating_sub(last_tick.elapsed());

            tokio::select! {
                // Check for new events from proxy
                Some(req_event) = event_rx.recv() => {
                    self.add_request(&req_event);
                    // Forward to archive writer
                    let _ = archive_tx.send(req_event).await;
                }

                // Check for keyboard input
                _ = tokio::time::sleep(timeout) => {
                    if event::poll(Duration::ZERO)? {
                        if let Event::Key(key) = event::read()? {
                            if key.kind == KeyEventKind::Press {
                                match key.code {
                                    KeyCode::Char('q') | KeyCode::Esc => {
                                        break;
                                    }
                                    KeyCode::Char('c') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }

            if last_tick.elapsed() >= tick_rate {
                last_tick = Instant::now();
            }
        }

        restore_terminal(&mut terminal)?;
        Ok(())
    }

    fn add_request(&mut self, event: &RequestEvent) {
        self.total_tokens += event.tokens as u64;
        self.last_provider = event.provider.clone();

        if let Some(prompt) = event.last_user_message() {
            self.last_prompt = prompt.to_string();
        }

        let info = RequestInfo::from(event);
        self.requests.push_front(info);

        // Keep only max_log_entries
        while self.requests.len() > self.config.max_log_entries {
            self.requests.pop_back();
        }
    }

    fn render(&self, frame: &mut Frame) {
        let chunks = Layout::vertical([
            Constraint::Length(3),  // Header
            Constraint::Length(5),  // Fuel gauge
            Constraint::Min(10),    // Request log
            Constraint::Length(6),  // Last prompt
        ])
        .split(frame.area());

        frame.render_widget(self.header(), chunks[0]);
        frame.render_widget(self.fuel_gauge(), chunks[1]);
        frame.render_widget(self.request_table(chunks[2]), chunks[2]);
        frame.render_widget(self.prompt_panel(), chunks[3]);
    }

    fn header(&self) -> Paragraph<'_> {
        let title = if self.last_provider.is_empty() {
            "SHERLOCK - LLM Traffic Inspector".to_string()
        } else {
            format!(
                "SHERLOCK - LLM Traffic Inspector ({})",
                self.last_provider.to_uppercase()
            )
        };

        Paragraph::new(Line::from(vec![Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]))
        .block(Block::default().borders(Borders::ALL))
        .alignment(ratatui::layout::Alignment::Center)
    }

    fn fuel_gauge(&self) -> Gauge<'_> {
        let percentage =
            (self.total_tokens as f64 / self.config.token_limit as f64 * 100.0).min(100.0);

        let color = if percentage < 50.0 {
            Color::Green
        } else if percentage < 80.0 {
            Color::Yellow
        } else {
            Color::Red
        };

        let label = format!(
            "{} / {} tokens ({:.1}%)",
            format_number(self.total_tokens),
            format_number(self.config.token_limit),
            percentage
        );

        Gauge::default()
            .block(
                Block::default()
                    .title(" Context Usage ")
                    .borders(Borders::ALL),
            )
            .gauge_style(Style::default().fg(color))
            .percent(percentage as u16)
            .label(label)
    }

    fn request_table(&self, _area: Rect) -> Table<'_> {
        let header = Row::new(vec!["Time", "Provider", "Model", "Tokens"])
            .style(Style::default().add_modifier(Modifier::BOLD))
            .bottom_margin(1);

        let rows: Vec<Row> = self
            .requests
            .iter()
            .map(|r| {
                Row::new(vec![
                    r.time.clone(),
                    r.provider.clone(),
                    truncate(&r.model, 30),
                    format_number(r.tokens as u64),
                ])
            })
            .collect();

        Table::new(
            rows,
            [
                Constraint::Length(10),
                Constraint::Length(12),
                Constraint::Min(20),
                Constraint::Length(12),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .title(format!(" Request Log ({}) ", self.requests.len()))
                .borders(Borders::ALL),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
    }

    fn prompt_panel(&self) -> Paragraph<'_> {
        let preview = if self.last_prompt.is_empty() {
            "No prompts yet...".to_string()
        } else {
            truncate(&self.last_prompt, self.config.prompt_preview_length)
        };

        Paragraph::new(preview)
            .block(
                Block::default()
                    .title(" Last Prompt ")
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::White))
    }
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn truncate(s: &str, max_len: usize) -> String {
    // Single-pass truncation: collect up to max_len char boundaries
    let trunc_at = max_len.saturating_sub(3);
    let mut trunc_byte_idx = 0;
    let mut end_byte_idx = 0;

    for (i, (byte_idx, _)) in s.char_indices().enumerate() {
        if i == trunc_at {
            trunc_byte_idx = byte_idx;
        }
        if i == max_len {
            // String exceeds max_len, truncate
            return format!("{}...", &s[..trunc_byte_idx]);
        }
        end_byte_idx = byte_idx;
    }
    // Check if we need the last character's end
    let _ = end_byte_idx;
    s.to_string()
}

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.insert(0, ',');
        }
        result.insert(0, c);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(100), "100");
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1234567), "1,234,567");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 8), "hello...");
    }
}
