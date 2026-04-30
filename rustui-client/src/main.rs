use std::time::Duration;

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures_util::{SinkExt, StreamExt};
use rustui_client::{
    App, FocusedSection, LoginState, Spans, crypto::Crypto, draw_chat_screen, draw_login_screen,
    get_timestamp,
};
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();

    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = tui::backend::CrosstermBackend::new(stdout);
    let mut terminal = tui::Terminal::new(backend)?;

    let mut login_state = LoginState::new();
    let mut authenticated = false;
    let mut username = String::new();

    // Login screen loop
    loop {
        terminal.draw(|f| {
            let size = f.size();
            draw_login_screen(f, size, &login_state);
        })?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Tab => {
                            login_state.active_field = (login_state.active_field + 1) % 4;
                        }
                        KeyCode::Char(c) => {
                            match login_state.active_field {
                                0 => login_state.server_address.push(c),
                                1 => login_state.username.push(c),
                                2 => login_state.password.push(c),
                                3 => login_state.encryption_key.push(c),
                                _ => {}
                            }
                            login_state.error.clear();
                        }
                        KeyCode::Backspace => match login_state.active_field {
                            0 => {
                                login_state.server_address.pop();
                            }
                            1 => {
                                login_state.username.pop();
                            }
                            2 => {
                                login_state.password.pop();
                            }
                            3 => {
                                login_state.encryption_key.pop();
                            }
                            _ => {}
                        },
                        KeyCode::Enter => {
                            if !login_state.server_address.is_empty()
                                && !login_state.username.is_empty()
                                && !login_state.password.is_empty()
                                && !login_state.encryption_key.is_empty()
                            {
                                break;
                            } else {
                                login_state.error = "All fields are required".to_string();
                            }
                        }
                        KeyCode::Esc => {
                            disable_raw_mode()?;
                            execute!(
                                terminal.backend_mut(),
                                LeaveAlternateScreen,
                                DisableMouseCapture
                            )?;
                            terminal.show_cursor()?;
                            return Ok(());
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // Connect to server
    let server_address = &login_state.server_address;
    let (ws_stream, _) = match connect_async(server_address).await {
        Ok(stream) => stream,
        Err(e) => {
            disable_raw_mode()?;
            execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture
            )?;
            terminal.show_cursor()?;
            eprintln!("Failed to connect to {}: {}", server_address, e);
            return Err(e.into());
        }
    };

    let (mut write, mut read) = ws_stream.split();

    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(100);

    tokio::spawn(async move {
        while let Some(msg) = read.next().await {
            if let Ok(Message::Text(text)) = msg {
                let _ = tx.send(text).await;
            }
        }
    });

    // Send authentication message immediately after connection
    let auth_msg = serde_json::json!({
        "Auth": { "username": &login_state.username, "password": &login_state.password }
    });
    write.send(Message::Text(auth_msg.to_string())).await?;

    // Authentication loop - wait for server response
    let mut pending_messages: Vec<String> = Vec::new();
    loop {
        terminal.draw(|f| {
            let size = f.size();
            draw_login_screen(f, size, &login_state);
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
                        let err_msg = json
                            .get("msg")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown error");
                        login_state.error = err_msg.to_string();
                    }
                    _ => {
                        // Store unhandled messages for processing after authentication
                        pending_messages.push(msg);
                    }
                }
            }
        }

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Esc => {
                            disable_raw_mode()?;
                            execute!(
                                terminal.backend_mut(),
                                LeaveAlternateScreen,
                                DisableMouseCapture
                            )?;
                            terminal.show_cursor()?;
                            return Ok(());
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

    // Create crypto instance with the encryption key
    let crypto = Crypto::new(&login_state.encryption_key);

    // Process any pending messages that arrived during authentication
    for msg in pending_messages {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&msg) {
            let msg_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
            match msg_type {
                "list" => {
                    if let Some(clients) = json.get("clients").and_then(|v| v.as_array()) {
                        let ids: Vec<String> = clients
                            .iter()
                            .filter_map(|c| c.as_str().map(String::from))
                            .collect();
                        app.set_participants(ids);
                    }
                }
                _ => {}
            }
        }
    }

    loop {
        terminal.draw(|f| {
            let size = f.size();
            draw_chat_screen(f, size, &mut app);
        })?;

        // Process ALL available messages, not just one
        while let Some(msg) = rx.try_recv().ok() {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&msg) {
                let msg_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match msg_type {
                    "message" => {
                        let from = json
                            .get("from")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let encrypted_text = json.get("msg").and_then(|v| v.as_str()).unwrap_or("");
                        let ts = get_timestamp();

                        // Try to decrypt the message
                        let display_text = match crypto.decrypt(encrypted_text) {
                            Ok(decrypted) => decrypted,
                            Err(_) => format!("[encrypted: {}]", encrypted_text),
                        };

                        app.add_message(format!("[{}] {}: {}", ts, from, display_text));
                    }
                    "list" => {
                        if let Some(clients) = json.get("clients").and_then(|v| v.as_array()) {
                            let ids: Vec<String> = clients
                                .iter()
                                .filter_map(|c| c.as_str().map(String::from))
                                .collect();
                            app.set_participants(ids);
                        }
                    }
                    "error" => {
                        let err_msg = json
                            .get("msg")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown error");
                        app.add_message(format!("[system] Error: {}", err_msg));
                    }
                    "system" => {
                        let ts = get_timestamp();
                        let sys_msg = json.get("msg").and_then(|v| v.as_str()).unwrap_or("");
                        app.add_message(format!("[{}] system: {}", ts, sys_msg));
                    }
                    _ => {}
                }
            }
        }

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    // Handle Shift+Tab to toggle focus
                    if key.code == KeyCode::BackTab
                        || (key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::SHIFT))
                    {
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

                                        // Encrypt the message before sending
                                        match crypto.encrypt(&input) {
                                            Ok(encrypted) => {
                                                // Show plaintext in our own chat
                                                let msg =
                                                    format!("[{}] {}: {}", ts, app.username, input);
                                                app.messages.push(Spans::from(msg));

                                                // Send encrypted message to server
                                                let send_msg = serde_json::json!({
                                                    "Broadcast": { "msg": encrypted }
                                                });
                                                let _ = write
                                                    .send(Message::Text(send_msg.to_string()))
                                                    .await;
                                            }
                                            Err(e) => {
                                                app.add_message(format!(
                                                    "[system] Encryption error: {}",
                                                    e
                                                ));
                                            }
                                        }

                                        app.input.clear();
                                        app.input_cursor_pos = 0;
                                    }
                                }
                                KeyCode::Esc => break,
                                _ => {}
                            }
                        }
                        FocusedSection::MessageList => match key.code {
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
                        },
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
