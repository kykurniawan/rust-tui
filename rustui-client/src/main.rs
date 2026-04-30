use std::time::Duration;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use rustui_client::{App, Spans, LoginState, FocusedSection, draw_chat_screen, draw_login_screen, get_timestamp};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = "ws://127.0.0.1:8080";
    let (ws_stream, _) = connect_async(url).await?;
    println!("Connected to server!");

    let (mut write, mut read) = ws_stream.split();

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();

    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = tui::backend::CrosstermBackend::new(stdout);
    let mut terminal = tui::Terminal::new(backend)?;

    let mut login_state = LoginState::new();
    let mut authenticated = false;
    let mut username = String::new();

    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(100);

    tokio::spawn(async move {
        while let Some(msg) = read.next().await {
            if let Ok(Message::Text(text)) = msg {
                let _ = tx.send(text).await;
            }
        }
    });

    loop {
        terminal.draw(|f| {
            let size = f.size();
            draw_login_screen(f, size, &mut login_state);
        })?;

        while let Some(msg) = rx.try_recv().ok() {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&msg) {
                let msg_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match msg_type {
                    "authenticated" => {
                        if let Some(user) = json.get("username").and_then(|v| v.as_str()) {
                            username = user.to_string();
                            authenticated = true;
                        }
                    }
                    "error" => {
                        let err_msg = json.get("msg").and_then(|v| v.as_str()).unwrap_or("Unknown error");
                        login_state.error = err_msg.to_string();
                    }
                    _ => {}
                }
            }
        }

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Tab => {
                            login_state.active_field = if login_state.active_field == 0 { 1 } else { 0 };
                        }
                        KeyCode::Char(c) => {
                            if login_state.active_field == 0 {
                                login_state.username.push(c);
                            } else {
                                login_state.password.push(c);
                            }
                            login_state.error.clear();
                        }
                        KeyCode::Backspace => {
                            if login_state.active_field == 0 {
                                login_state.username.pop();
                            } else {
                                login_state.password.pop();
                            }
                        }
                        KeyCode::Enter => {
                            if !login_state.username.is_empty() && !login_state.password.is_empty() {
                                let auth_msg = serde_json::json!({
                                    "Auth": { "username": &login_state.username, "password": &login_state.password }
                                });
                                write.send(Message::Text(auth_msg.to_string())).await?;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        if authenticated {
            break;
        }
    }

    let mut app = App::new();
    app.init(username.clone());

    loop {
        terminal.draw(|f| {
            let size = f.size();
            draw_chat_screen(f, size, &mut app);
        })?;

        if let Some(msg) = rx.try_recv().ok() {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&msg) {
                let msg_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match msg_type {
                    "message" => {
                        let from = json.get("from").and_then(|v| v.as_str()).unwrap_or("unknown");
                        let text = json.get("msg").and_then(|v| v.as_str()).unwrap_or("");
                        let ts = get_timestamp();
                        app.add_message(format!("[{}] {}: {}", ts, from, text));
                    }
                    "list" => {
                        if let Some(clients) = json.get("clients").and_then(|v| v.as_array()) {
                            let ids: Vec<String> = clients.iter()
                                .filter_map(|c| c.as_str().map(String::from))
                                .collect();
                            app.set_participants(ids);
                        }
                    }
                    "error" => {
                        let err_msg = json.get("msg").and_then(|v| v.as_str()).unwrap_or("Unknown error");
                        app.add_message(format!("[system] Error: {}", err_msg));
                    }
                    _ => {}
                }
            }
        }

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    // Handle Shift+Tab to toggle focus
                    if key.code == KeyCode::BackTab || 
                       (key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::SHIFT)) {
                        app.toggle_focus();
                        continue;
                    }

                    match app.focus {
                        FocusedSection::Input => {
                            // Get terminal width for cursor movement calculations
                            let term_size = terminal.size()?;
                            let input_width = term_size.width.saturating_sub(4) as usize;

                            match key.code {
                                KeyCode::Char(c) => {
                                    app.insert_char(c);
                                }
                                KeyCode::Backspace => {
                                    app.delete_char();
                                }
                                KeyCode::Delete => {
                                    app.delete_char_forward();
                                }
                                KeyCode::Left => {
                                    app.move_cursor_left();
                                }
                                KeyCode::Right => {
                                    app.move_cursor_right();
                                }
                                KeyCode::Up => {
                                    app.move_cursor_up(input_width);
                                }
                                KeyCode::Down => {
                                    app.move_cursor_down(input_width);
                                }
                                KeyCode::Home => {
                                    app.move_cursor_to_start();
                                }
                                KeyCode::End => {
                                    app.move_cursor_to_end();
                                }
                                KeyCode::Enter => {
                                    if !app.input.is_empty() && authenticated {
                                        let input = app.input.trim().to_string();
                                        let ts = get_timestamp();
                                        let msg = format!("[{}] {}: {}", ts, app.username, input);
                                        app.messages.push(Spans::from(msg));
                                        
                                        let send_msg = serde_json::json!({
                                            "Broadcast": { "msg": input }
                                        });
                                        let _ = write.send(Message::Text(send_msg.to_string())).await;
                                        
                                        app.input.clear();
                                        app.input_cursor_pos = 0;
                                    }
                                }
                                KeyCode::Esc => break,
                                _ => {}
                            }
                        }
                        FocusedSection::MessageList => {
                            match key.code {
                                KeyCode::Up => {
                                    app.scroll_up();
                                }
                                KeyCode::Down => {
                                    app.scroll_down();
                                }
                                KeyCode::PageUp => {
                                    for _ in 0..10 {
                                        app.scroll_up();
                                    }
                                }
                                KeyCode::PageDown => {
                                    for _ in 0..10 {
                                        app.scroll_down();
                                    }
                                }
                                KeyCode::Home => {
                                    app.message_scroll = 0;
                                    app.auto_scroll = false;
                                }
                                KeyCode::End => {
                                    app.scroll_to_bottom();
                                }
                                KeyCode::Esc => break,
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
