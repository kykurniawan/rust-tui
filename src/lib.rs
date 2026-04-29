pub use tui::text::Spans;
pub use tui::style::Style;

use std::time::{SystemTime, UNIX_EPOCH};

use tui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Color,
    text::{Span, Text},
    widgets::{Block, Borders, Paragraph, List, ListItem, Wrap},
    Frame,
};

pub fn get_timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let hours = (secs / 3600) % 24;
    let mins = (secs / 60) % 60;
    let secs = secs % 60;
    format!("{:02}:{:02}:{:02}", hours, mins, secs)
}



pub struct App {
    pub username: String,
    pub messages: Vec<Spans<'static>>,
    pub input: String,
    pub input_scroll: u16,
    pub participants: Vec<String>,
    pub authenticated: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            username: String::new(),
            messages: vec![],
            input: String::new(),
            input_scroll: 0,
            participants: vec![],
            authenticated: false,
        }
    }

    pub fn init(&mut self, username: String) {
        self.username = username.clone();
let ts = get_timestamp();
        self.messages = vec![
            Spans::from(vec![
                Span::raw("["),
                Span::styled(ts, Style::default().fg(Color::DarkGray)),
                Span::raw("] "),
                Span::styled("system", Style::default().fg(Color::Cyan).add_modifier(tui::style::Modifier::BOLD)),
                Span::raw(":"),
                Span::raw(" Welcome "),
                Span::styled(username.clone(), Style::default().fg(Color::Yellow).add_modifier(tui::style::Modifier::BOLD)),
                Span::raw("! Type a message to broadcast."),
            ]),
        ];
        self.authenticated = true;
    }

    pub fn add_message(&mut self, msg: String) {
        self.messages.push(Spans::from(msg));
    }

    pub fn set_participants(&mut self, participants: Vec<String>) {
        self.participants = participants;
    }
}

pub struct LoginState {
    pub username: String,
    pub password: String,
    pub active_field: u8,
    pub error: String,
}

impl LoginState {
    pub fn new() -> Self {
        Self {
            username: String::new(),
            password: String::new(),
            active_field: 0,
            error: String::new(),
        }
    }
}

pub fn draw_login_screen<W: std::io::Write>(f: &mut Frame<CrosstermBackend<W>>, area: Rect, state: &LoginState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area);

    let title = Paragraph::new(
        Text::from("SECURE CHAT LOGIN")
    )
    .block(Block::default().title("").borders(Borders::NONE))
    .style(Style::default().fg(Color::Yellow).add_modifier(tui::style::Modifier::BOLD))
    .alignment(Alignment::Center);
    f.render_widget(title, chunks[0]);

    let form_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(chunks[1]);

    let username_style = if state.active_field == 0 {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::White)
    };
    let username_input = Paragraph::new(state.username.as_str())
        .block(
            Block::default()
                .title(" Username ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(if state.active_field == 0 { Color::Green } else { Color::DarkGray }))
        )
        .style(username_style);
    f.render_widget(username_input, form_chunks[0]);

    let password_display: String = state.password.chars().map(|_| '*').collect();
    let password_style = if state.active_field == 1 {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::White)
    };
    let password_input = Paragraph::new(password_display.as_str())
        .block(
            Block::default()
                .title(" Password ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(if state.active_field == 1 { Color::Green } else { Color::DarkGray }))
        )
        .style(password_style);
    f.render_widget(password_input, form_chunks[1]);

    let help = Paragraph::new(
        Text::from("Press TAB to switch fields | ENTER to login")
    )
    .block(Block::default().borders(Borders::NONE))
    .style(Style::default().fg(Color::DarkGray))
    .alignment(Alignment::Center);
    f.render_widget(help, form_chunks[2]);

    if !state.error.is_empty() {
        let error_block = Paragraph::new(state.error.as_str())
            .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Red)))
            .style(Style::default().fg(Color::Red))
            .alignment(Alignment::Center);
        f.render_widget(error_block, chunks[2]);
    }

    if state.active_field == 0 {
        let cursor_x = form_chunks[0].x + 1 + state.username.len().min(form_chunks[0].width as usize - 2) as u16;
        f.set_cursor(cursor_x, form_chunks[0].y + 1);
    } else {
        let cursor_x = form_chunks[1].x + 1 + state.password.len().min(form_chunks[1].width as usize - 2) as u16;
        f.set_cursor(cursor_x, form_chunks[1].y + 1);
    }
}

pub fn draw_chat_screen<W: std::io::Write>(f: &mut Frame<CrosstermBackend<W>>, area: Rect, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(5),
        ])
        .split(area);

    let status = if app.authenticated {
        format!("Logged in as {} | {} participants", app.username, app.participants.len())
    } else {
        "Not authenticated".to_string()
    };

    let header = Paragraph::new(
        Text::from(vec![Spans::from(vec![
            Span::raw(">> SECURE_CHAT | "),
            Span::styled(status, Style::default().fg(Color::Green).add_modifier(tui::style::Modifier::BOLD)),
        ])])
    )
    .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)))
    .style(Style::default().fg(Color::White));
    f.render_widget(header, chunks[0]);

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(20),
            Constraint::Min(0),
        ])
        .split(chunks[1]);

let participant_items: Vec<ListItem> = app
        .participants
        .iter()
        .map(|id| {
            ListItem::new(id.as_str())
                .style(Style::default().fg(Color::White))
        })
        .collect();

    let participant_list = List::new(participant_items)
        .block(Block::default().title(" PARTICIPANTS ").borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)))
        .style(Style::default().fg(Color::White));

    f.render_widget(participant_list, main_chunks[0]);

    let message_area = Paragraph::new(Text::from(app.messages.clone()))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
        )
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: true });

    f.render_widget(message_area, main_chunks[1]);

    let input_block = Paragraph::new(app.input.as_str())
        .block(
            Block::default()
                .title(" MESSAGE >> ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green))
        )
        .style(Style::default().fg(Color::Green))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true })
        .scroll((app.input_scroll, 0));

    f.render_widget(input_block, chunks[2]);

    let input_area = chunks[2];
    if input_area.width > 2 && input_area.height > 2 {
        let line_width = (input_area.width - 2) as usize;
        let input_len = app.input.len();
        let current_line = if line_width > 0 { input_len / line_width } else { 0 };
        let visible_lines = (input_area.height - 2) as usize;

        if current_line >= (app.input_scroll as usize + visible_lines) {
            app.input_scroll = ((current_line + 1).saturating_sub(visible_lines)) as u16;
        } else if current_line < app.input_scroll as usize && app.input_scroll > 0 {
            app.input_scroll = current_line as u16;
        }

        let cursor_col = if line_width > 0 { input_len % line_width } else { 0 };
        let cursor_row = current_line.saturating_sub(app.input_scroll as usize);
        let cursor_x = input_area.x + 1 + cursor_col as u16;
        let cursor_y = input_area.y + 1 + cursor_row as u16;
        
        f.set_cursor(cursor_x, cursor_y);
    }
}