use std::{io, time::Duration};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use tui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style},
    symbols::DOT,
    text::{Span, Spans},
    widgets::{Block, Borders, Paragraph, Tabs},
    Terminal,
};

fn main() -> Result<(), io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();

    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 👇 App state
    let mut active_tab = 0;
    let tab_titles = ["Tab1", "Tab2", "Tab3", "Tab4"];
    let tab_count = tab_titles.len();

    loop {
        terminal.draw(|f| {
            let size = f.size();

            // Split layout: tabs (top) + content (bottom)
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(0),
                ])
                .split(size);

            // Tabs
            let titles: Vec<Spans> = tab_titles
                .iter()
                .map(|t| Spans::from(Span::raw(*t)))
                .collect();

            let tabs = Tabs::new(titles)
                .block(
                    Block::default()
                        .title("My Terminal UI")
                        .title_alignment(Alignment::Center)
                        .borders(Borders::ALL),
                )
                .style(Style::default().fg(Color::White))
                .highlight_style(Style::default().fg(Color::Yellow))
                .select(active_tab)
                .divider(DOT);

            f.render_widget(tabs, chunks[0]);

            // Content based on active tab
            let content = match active_tab {
                0 => "This is Tab 1 content",
                1 => "Welcome to Tab 2 🚀",
                2 => "You're viewing Tab 3",
                3 => "This is Tab 4",
                _ => "",
            };

            let paragraph = Paragraph::new(content)
                .block(Block::default().borders(Borders::ALL).title("Content"));

            f.render_widget(paragraph, chunks[1]);
        })?;

        // Input handling
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Right => {
                        active_tab = (active_tab + 1) % tab_count;
                    }
                    KeyCode::Left => {
                        active_tab = (active_tab + tab_count - 1) % tab_count;
                    }
                    _ => {}
                }
            }
        }
    }

    // Cleanup
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}