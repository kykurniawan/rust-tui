pub use tui::style::Style;
pub use tui::text::Spans;

pub mod crypto;

use std::time::{SystemTime, UNIX_EPOCH};

use tui::{
    Frame,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Color,
    text::{Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};

fn days_in_month(year: u64, month: u64) -> u64 {
    match month {
        2 => {
            if (year % 400 == 0) || (year % 4 == 0 && year % 100 != 0) {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

pub fn get_timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let mut secs = now.as_secs();

    let ss = secs % 60;
    secs /= 60;
    let mm = secs % 60;
    secs /= 60;
    let hh = secs % 24;
    secs /= 24;

    let mut days = secs as u64;
    let mut year: u64 = 1970;
    loop {
        let days_in_year = if (year % 400 == 0) || (year % 4 == 0 && year % 100 != 0) {
            366
        } else {
            365
        };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }

    let mut month: u64 = 1;
    loop {
        let dim = days_in_month(year, month);
        if days < dim {
            break;
        }
        days -= dim;
        month += 1;
    }
    let dd = days + 1;

    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC", year, month, dd, hh, mm, ss)
}

pub struct App {
    pub server_address: String,
    pub username: String,
    pub room: String,
    pub messages: Vec<Spans<'static>>,
    pub input: String,
    pub input_scroll: u16,
    pub input_cursor_pos: usize,
    pub message_scroll: usize,
    pub participants: Vec<String>,
    pub authenticated: bool,
    pub connected: bool,
    pub auto_scroll: bool,
    pub focus: FocusedSection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedSection {
    MessageList,
    Input,
}

impl App {
    pub fn new() -> Self {
        Self {
            server_address: String::new(),
            username: String::new(),
            room: String::new(),
            messages: vec![],
            input: String::new(),
            input_scroll: 0,
            input_cursor_pos: 0,
            message_scroll: 0,
            participants: vec![],
            authenticated: false,
            connected: true,
            auto_scroll: true,
            focus: FocusedSection::Input,
        }
    }

    pub fn init(&mut self, server_address: String, username: String, room: String) {
        self.server_address = server_address.clone();
        self.username = username.clone();
        self.room = room.clone();
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
            Span::raw(" to "),
            Span::styled(
                room.clone(),
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(tui::style::Modifier::BOLD),
            ),
            Span::raw(" on "),
            Span::styled(
                server_address.clone(),
                Style::default()
                    .fg(Color::Green)
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

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            FocusedSection::MessageList => FocusedSection::Input,
            FocusedSection::Input => FocusedSection::MessageList,
        };
    }

    pub fn move_cursor_left(&mut self) {
        if self.input_cursor_pos > 0 {
            self.input_cursor_pos -= 1;
        }
    }

    pub fn move_cursor_right(&mut self) {
        let char_count = self.input.chars().count();
        if self.input_cursor_pos < char_count {
            self.input_cursor_pos += 1;
        }
    }

    pub fn move_cursor_up(&mut self, line_width: usize) {
        if line_width > 0 && self.input_cursor_pos >= line_width {
            self.input_cursor_pos = self.input_cursor_pos.saturating_sub(line_width);
        }
    }

    pub fn move_cursor_down(&mut self, line_width: usize) {
        let char_count = self.input.chars().count();
        if line_width > 0 {
            let new_pos = self.input_cursor_pos + line_width;
            if new_pos <= char_count {
                self.input_cursor_pos = new_pos;
            } else {
                self.input_cursor_pos = char_count;
            }
        }
    }

    pub fn insert_char(&mut self, c: char) {
        let byte_pos = self.input.chars().take(self.input_cursor_pos).map(|c| c.len_utf8()).sum();
        self.input.insert(byte_pos, c);
        self.input_cursor_pos += 1;
    }

    pub fn delete_char(&mut self) {
        if self.input_cursor_pos > 0 {
            let byte_pos = self.input.chars().take(self.input_cursor_pos - 1).map(|c| c.len_utf8()).sum();
            self.input.remove(byte_pos);
            self.input_cursor_pos -= 1;
        }
    }

    pub fn delete_char_forward(&mut self) {
        let char_count = self.input.chars().count();
        if self.input_cursor_pos < char_count {
            let byte_pos = self.input.chars().take(self.input_cursor_pos).map(|c| c.len_utf8()).sum();
            self.input.remove(byte_pos);
        }
    }

    pub fn move_cursor_to_start(&mut self) {
        self.input_cursor_pos = 0;
    }

    pub fn move_cursor_to_end(&mut self) {
        self.input_cursor_pos = self.input.chars().count();
    }
}

pub struct LoginState {
    pub server_address: String,
    pub room: String,
    pub username: String,
    pub password: String,
    pub encryption_key: String,
    pub active_field: u8,
    pub error: String,
}

impl LoginState {
    pub fn new() -> Self {
        Self {
            server_address: "ws://".to_string(),
            room: String::new(),
            username: String::new(),
            password: String::new(),
            encryption_key: String::new(),
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
            Constraint::Percentage(25),
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
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(chunks[1]);

    // Server Address field
    let server_style = if state.active_field == 0 {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::White)
    };
    let server_input = Paragraph::new(state.server_address.as_str())
        .block(
            Block::default()
                .title(" Server Address (ws://...) ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(if state.active_field == 0 {
                    Color::Green
                } else {
                    Color::DarkGray
                })),
        )
        .style(server_style);
    f.render_widget(server_input, form_chunks[0]);

    // Room field
    let room_style = if state.active_field == 1 {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::White)
    };
    let room_input = Paragraph::new(state.room.as_str())
        .block(
            Block::default()
                .title(" Room ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(if state.active_field == 1 {
                    Color::Green
                } else {
                    Color::DarkGray
                })),
        )
        .style(room_style);
    f.render_widget(room_input, form_chunks[1]);

    // Username field
    let username_style = if state.active_field == 2 {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::White)
    };
    let username_input = Paragraph::new(state.username.as_str())
        .block(
            Block::default()
                .title(" Username ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(if state.active_field == 2 {
                    Color::Green
                } else {
                    Color::DarkGray
                })),
        )
        .style(username_style);
    f.render_widget(username_input, form_chunks[2]);

    // Password field
    let password_display: String = state.password.chars().map(|_| '*').collect();
    let password_style = if state.active_field == 3 {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::White)
    };
    let password_input = Paragraph::new(password_display.as_str())
        .block(
            Block::default()
                .title(" Password ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(if state.active_field == 3 {
                    Color::Green
                } else {
                    Color::DarkGray
                })),
        )
        .style(password_style);
    f.render_widget(password_input, form_chunks[3]);

    // Encryption Key field
    let key_display: String = state.encryption_key.chars().map(|_| '*').collect();
    let key_style = if state.active_field == 4 {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::White)
    };
    let key_input = Paragraph::new(key_display.as_str())
        .block(
            Block::default()
                .title(" Encryption Key ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(if state.active_field == 4 {
                    Color::Green
                } else {
                    Color::DarkGray
                })),
        )
        .style(key_style);
    f.render_widget(key_input, form_chunks[4]);

    let help = Paragraph::new(Text::from("Press TAB to switch fields | ENTER to login"))
        .block(Block::default().borders(Borders::NONE))
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(help, form_chunks[5]);

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

    // Set cursor position based on active field
    match state.active_field {
        0 => {
            let cursor_x = form_chunks[0].x
                + 1
                + state.server_address.len().min(form_chunks[0].width as usize - 2) as u16;
            f.set_cursor(cursor_x, form_chunks[0].y + 1);
        }
        1 => {
            let cursor_x = form_chunks[1].x
                + 1
                + state.room.len().min(form_chunks[1].width as usize - 2) as u16;
            f.set_cursor(cursor_x, form_chunks[1].y + 1);
        }
        2 => {
            let cursor_x = form_chunks[2].x
                + 1
                + state.username.len().min(form_chunks[2].width as usize - 2) as u16;
            f.set_cursor(cursor_x, form_chunks[2].y + 1);
        }
        3 => {
            let cursor_x = form_chunks[3].x
                + 1
                + state.password.len().min(form_chunks[3].width as usize - 2) as u16;
            f.set_cursor(cursor_x, form_chunks[3].y + 1);
        }
        4 => {
            let cursor_x = form_chunks[4].x
                + 1
                + state.encryption_key.len().min(form_chunks[4].width as usize - 2) as u16;
            f.set_cursor(cursor_x, form_chunks[4].y + 1);
        }
        _ => {}
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
            Constraint::Length(7),
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
        format!("State: {} online", display_participants.len())
    } else {
        "Not authenticated".to_string()
    };

    let header = Paragraph::new(Text::from(vec![
        Spans::from(format!("Server: {}", app.server_address)),
        Spans::from(format!("Room: {}", app.room)),
        Spans::from(format!("User: {}", app.username)),
        Spans::from(if app.connected {
            format!("Status: Connected")
        } else {
            format!("Status: \u{26a0}\u{fe0f} Disconnected - reconnecting...")
        }),
        Spans::from(status),
    ]))
    .block(
        Block::default()
            .title(Spans::from(vec![Span::styled(
                " SECURE CHAT ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(tui::style::Modifier::BOLD),
            )]))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    )
    .style(Style::default().fg(Color::White))
    .wrap(Wrap { trim: false });
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

    let focus_indicator = if app.focus == FocusedSection::MessageList {
        " [A] "
    } else {
        " "
    };

    // Render the messages block
    let messages_block = Block::default()
        .title(format!(" MESSAGES{}{}", scroll_indicator, focus_indicator))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.focus == FocusedSection::MessageList {
            Color::Cyan
        } else {
            Color::DarkGray
        }));
    
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

        let display_text = format!("{}", plain_text);

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
                .title(if app.focus == FocusedSection::Input {
                    " MESSAGE >> [A] "
                } else {
                    " MESSAGE >> "
                })
                .borders(Borders::ALL)
                .border_style(Style::default().fg(if app.focus == FocusedSection::Input {
                    Color::Green
                } else {
                    Color::DarkGray
                })),
        )
        .style(Style::default().fg(if app.focus == FocusedSection::Input {
            Color::Green
        } else {
            Color::DarkGray
        }))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false })
        .scroll((app.input_scroll, 0));

    f.render_widget(input_block, chunks[2]);

    let input_area = chunks[2];
    if input_area.width > 2 && input_area.height > 2 && app.focus == FocusedSection::Input {
        let line_width = (input_area.width - 2) as usize;
        
        if line_width > 0 {
            // Use cursor position instead of input length
            let cursor_pos = app.input_cursor_pos;
            
            // Calculate which line the cursor is on
            let cursor_line = cursor_pos / line_width;
            let cursor_col = cursor_pos % line_width;
            
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
