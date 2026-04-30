pub use tui::style::Style;
pub use tui::text::Spans;

use std::time::{SystemTime, UNIX_EPOCH};

use tui::{
    Frame,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Color,
    text::{Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
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
    pub message_scroll: usize,
    pub participants: Vec<String>,
    pub authenticated: bool,
    pub auto_scroll: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            username: String::new(),
            messages: vec![],
            input: String::new(),
            input_scroll: 0,
            message_scroll: 0,
            participants: vec![],
            authenticated: false,
            auto_scroll: true,
        }
    }

    pub fn init(&mut self, username: String) {
        self.username = username.clone();
        let ts = get_timestamp();
        self.messages = vec![Spans::from(vec![
            Span::raw("["),
            Span::styled(ts, Style::default().fg(Color::DarkGray)),
            Span::raw("] "),
            Span::styled(
                "system",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(tui::style::Modifier::BOLD),
            ),
            Span::raw(":"),
            Span::raw(" Welcome "),
            Span::styled(
                username.clone(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(tui::style::Modifier::BOLD),
            ),
            Span::raw("! Type a message to broadcast."),
        ])];
        self.authenticated = true;
    }

    pub fn add_message(&mut self, msg: String) {
        self.messages.push(Spans::from(msg));
        if self.auto_scroll {
            self.message_scroll = self.messages.len().saturating_sub(1);
        }
    }

    pub fn set_participants(&mut self, participants: Vec<String>) {
        self.participants = participants;
    }

    pub fn scroll_up(&mut self) {
        if self.message_scroll > 0 {
            self.message_scroll = self.message_scroll.saturating_sub(1);
            self.auto_scroll = false;
        }
    }

    pub fn scroll_down(&mut self) {
        if self.message_scroll < self.messages.len().saturating_sub(1) {
            self.message_scroll = self.message_scroll.saturating_add(1);
            // Re-enable auto-scroll if we're at the bottom
            if self.message_scroll >= self.messages.len().saturating_sub(1) {
                self.auto_scroll = true;
            }
        }
    }

    pub fn scroll_to_bottom(&mut self) {
        self.message_scroll = self.messages.len().saturating_sub(1);
        self.auto_scroll = true;
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

pub fn draw_login_screen<W: std::io::Write>(
    f: &mut Frame<CrosstermBackend<W>>,
    area: Rect,
    state: &LoginState,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area);

    let title = Paragraph::new(Text::from("SECURE CHAT LOGIN"))
        .block(Block::default().title("").borders(Borders::NONE))
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(tui::style::Modifier::BOLD),
        )
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
                .border_style(Style::default().fg(if state.active_field == 0 {
                    Color::Green
                } else {
                    Color::DarkGray
                })),
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
                .border_style(Style::default().fg(if state.active_field == 1 {
                    Color::Green
                } else {
                    Color::DarkGray
                })),
        )
        .style(password_style);
    f.render_widget(password_input, form_chunks[1]);

    let help = Paragraph::new(Text::from("Press TAB to switch fields | ENTER to login"))
        .block(Block::default().borders(Borders::NONE))
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(help, form_chunks[2]);

    if !state.error.is_empty() {
        let error_block = Paragraph::new(state.error.as_str())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Red)),
            )
            .style(Style::default().fg(Color::Red))
            .alignment(Alignment::Center);
        f.render_widget(error_block, chunks[2]);
    }

    if state.active_field == 0 {
        let cursor_x = form_chunks[0].x
            + 1
            + state.username.len().min(form_chunks[0].width as usize - 2) as u16;
        f.set_cursor(cursor_x, form_chunks[0].y + 1);
    } else {
        let cursor_x = form_chunks[1].x
            + 1
            + state.password.len().min(form_chunks[1].width as usize - 2) as u16;
        f.set_cursor(cursor_x, form_chunks[1].y + 1);
    }
}

pub fn draw_chat_screen<W: std::io::Write>(
    f: &mut Frame<CrosstermBackend<W>>,
    area: Rect,
    app: &mut App,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(5),
        ])
        .split(area);

    let status = if app.authenticated {
        let self_in_list = app.participants.iter().any(|p| p == &app.username);
        let display_participants = if self_in_list {
            app.participants.clone()
        } else {
            let mut p = app.participants.clone();
            p.insert(0, app.username.clone());
            p
        };
        format!(
            "Logged in as {} | {} online",
            app.username,
            display_participants.len()
        )
    } else {
        "Not authenticated".to_string()
    };

    let header = Paragraph::new(Text::from(vec![Spans::from(vec![
        Span::raw(">> SECURE_CHAT | "),
        Span::styled(
            status,
            Style::default()
                .fg(Color::Green)
                .add_modifier(tui::style::Modifier::BOLD),
        ),
    ])]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    )
    .style(Style::default().fg(Color::White));
    f.render_widget(header, chunks[0]);

    // Calculate how many lines each message will take
    let msg_area_width = chunks[1].width.saturating_sub(4) as usize; // Account for borders and padding
    let mut message_heights: Vec<usize> = Vec::new();
    
    for msg in &app.messages {
        let plain_text: String = msg.0.iter().map(|s| s.content.to_string()).collect();
        let text_len = plain_text.len();
        
        // Calculate wrapped lines (minimum 1 line per message)
        let lines = if msg_area_width > 0 {
            ((text_len + msg_area_width - 1) / msg_area_width).max(1)
        } else {
            1
        };
        message_heights.push(lines);
    }

    let total_messages = app.messages.len();
    let visible_height = chunks[1].height.saturating_sub(2) as usize;

    // Calculate which messages can fit in the visible area
    let mut visible_messages: Vec<usize> = Vec::new();
    let mut current_height = 0;
    
    // Start from the selected message and work backwards to fill the view
    let start_search = app.message_scroll.min(total_messages.saturating_sub(1));
    
    // First, try to show the selected message and messages after it
    for i in start_search..total_messages {
        let msg_height = message_heights[i];
        if current_height + msg_height <= visible_height {
            visible_messages.push(i);
            current_height += msg_height;
        } else {
            break;
        }
    }
    
    // Then, add messages before the selected one if there's space
    if start_search > 0 {
        for i in (0..start_search).rev() {
            let msg_height = message_heights[i];
            if current_height + msg_height <= visible_height {
                visible_messages.insert(0, i);
                current_height += msg_height;
            } else {
                break;
            }
        }
    }

    let scroll_indicator = if total_messages > 0 {
        format!(" [{}/{}] ↑↓ scroll | PgUp/PgDn | END to bottom ", 
            app.message_scroll + 1, 
            total_messages)
    } else {
        String::new()
    };

    // Render the messages block
    let messages_block = Block::default()
        .title(format!(" MESSAGES{} ", scroll_indicator))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    
    f.render_widget(messages_block, chunks[1]);

    // Render individual messages inside the block
    let inner_area = Rect {
        x: chunks[1].x + 1,
        y: chunks[1].y + 1,
        width: chunks[1].width.saturating_sub(2),
        height: chunks[1].height.saturating_sub(2),
    };

    let mut y_offset = 0;
    for &msg_idx in &visible_messages {
        if y_offset >= inner_area.height as usize {
            break;
        }

        let msg = &app.messages[msg_idx];
        let plain_text: String = msg.0.iter().map(|s| s.content.to_string()).collect();
        let msg_height = message_heights[msg_idx];
        let is_selected = msg_idx == app.message_scroll;

        let style = if is_selected {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(tui::style::Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let prefix = if is_selected { ">> " } else { "   " };
        let display_text = format!("{}{}", prefix, plain_text);

        let paragraph = Paragraph::new(display_text)
            .style(style)
            .wrap(Wrap { trim: false });

        let msg_rect = Rect {
            x: inner_area.x,
            y: inner_area.y + y_offset as u16,
            width: inner_area.width,
            height: msg_height.min((inner_area.height as usize).saturating_sub(y_offset)) as u16,
        };

        f.render_widget(paragraph, msg_rect);
        y_offset += msg_height;
    }

    let input_block = Paragraph::new(app.input.as_str())
        .block(
            Block::default()
                .title(" MESSAGE >> ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green)),
        )
        .style(Style::default().fg(Color::Green))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false })
        .scroll((app.input_scroll, 0));

    f.render_widget(input_block, chunks[2]);

    let input_area = chunks[2];
    if input_area.width > 2 && input_area.height > 2 {
        let line_width = (input_area.width - 2) as usize;
        
        if line_width > 0 {
            // Count actual characters (not bytes)
            let char_count = app.input.chars().count();
            
            // Calculate which line the cursor is on
            let cursor_line = char_count / line_width;
            let cursor_col = char_count % line_width;
            
            // Calculate visible lines in the input area
            let visible_lines = (input_area.height - 2) as usize;
            
            // Auto-scroll the input if cursor goes beyond visible area
            if cursor_line >= visible_lines {
                app.input_scroll = (cursor_line - visible_lines + 1) as u16;
            } else if cursor_line < app.input_scroll as usize {
                app.input_scroll = cursor_line as u16;
            }
            
            // Calculate cursor position relative to the scrolled view
            let visible_cursor_line = cursor_line.saturating_sub(app.input_scroll as usize);
            
            // Set cursor position
            let cursor_x = input_area.x + 1 + cursor_col as u16;
            let cursor_y = input_area.y + 1 + visible_cursor_line as u16;
            
            // Make sure cursor is within bounds
            if cursor_x < input_area.x + input_area.width - 1 
                && cursor_y < input_area.y + input_area.height - 1 {
                f.set_cursor(cursor_x, cursor_y);
            }
        }
    }
}
