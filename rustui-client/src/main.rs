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
    let mut room = String::new();

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
                            login_state.active_field = (login_state.active_field + 1) % 5;
                        }
                        KeyCode::Char(c) => {
                            match login_state.active_field {
                                0 => login_state.server_address.push(c),
                                1 => login_state.room.push(c),
                                2 => login_state.username.push(c),
                                3 => login_state.password.push(c),
                                4 => login_state.encryption_key.push(c),
                                _ => {}
                            }
                            login_state.error.clear();
                        }
                        KeyCode::Backspace => match login_state.active_field {
                            0 => {
                                login_state.server_address.pop();
                            }
                            1 => {
                                login_state.room.pop();
                            }
                            2 => {
                                login_state.username.pop();
                            }
                            3 => {
                                login_state.password.pop();
                            }
                            4 => {
                                login_state.encryption_key.pop();
                            }
                            _ => {}
                        },
                        KeyCode::Enter => {
                            if !login_state.server_address.is_empty()
                                && !login_state.room.is_empty()
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
    let ws_url = format!("{}/room/{}", server_address, login_state.room);
    let (ws_stream, _) = match connect_async(&ws_url).await {
        Ok(stream) => stream,
        Err(e) => {
            disable_raw_mode()?;
            execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture
            )?;
            terminal.show_cursor()?;
            eprintln!("Failed to connect to {}: {}", ws_url, e);
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
                            room = json
                                .get("room")
                                .and_then(|v| v.as_str())
                                .unwrap_or("?")
                                .to_string();
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
    app.init(
        login_state.server_address.clone(),
        username.clone(),
        room.clone(),
    );

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

    // Store credentials for reconnection
    let auth_username = login_state.username.clone();
    let auth_password = login_state.password.clone();
    let reconnect_url = format!("{}/room/{}", login_state.server_address, login_state.room);
    let mut last_heartbeat = std::time::Instant::now();

    // Outer reconnection loop
    'session: loop {
        if !app.connected {
            app.add_message("[system] Reconnecting...".to_string());
            app.scroll_to_bottom();

            // Try to reconnect with exponential backoff
            let mut retry_delay = 1u64;
            loop {
                match connect_async(&reconnect_url).await {
                    Ok((new_stream, _)) => {
                        let (new_write, mut new_read) = new_stream.split();
                        write = new_write;

                        let (new_tx, new_rx) = tokio::sync::mpsc::channel::<String>(100);
                        tokio::spawn(async move {
                            while let Some(msg) = new_read.next().await {
                                if let Ok(Message::Text(text)) = msg {
                                    let _ = new_tx.send(text).await;
                                }
                            }
                        });
                        rx = new_rx;

                        // Re-authenticate
                        let auth_msg = serde_json::json!({
                            "Auth": { "username": &auth_username, "password": &auth_password }
                        });
                        match write.send(Message::Text(auth_msg.to_string())).await {
                            Ok(_) => {
                                let wait_start = std::time::Instant::now();
                                while wait_start.elapsed() < std::time::Duration::from_secs(5) {
                                    if let Some(msg) = rx.try_recv().ok() {
                                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&msg) {
                                            if json.get("type").and_then(|v| v.as_str()) == Some("authenticated") {
                                                app.connected = true;
                                                app.add_message("[system] Reconnected".to_string());
                                                break;
                                            }
                                        }
                                    }
                                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                                }

                                if app.connected {
                                    last_heartbeat = std::time::Instant::now();
                                    break; // out of retry loop, back to chat
                                }
                            }
                            Err(_) => {}
                        }
                    }
                    Err(_) => {}
                }

                // Exponential backoff before next retry
                app.add_message(format!(
                    "[system] Reconnect failed, retrying in {}s...", retry_delay
                ));
                app.scroll_to_bottom();
                tokio::time::sleep(std::time::Duration::from_secs(retry_delay)).await;
                retry_delay = std::cmp::min(retry_delay * 2, 30);
            }
        }

        // Inner chat loop
        'chat: loop {
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

            // Heartbeat every 30s to keep connection alive through proxies
            if last_heartbeat.elapsed() >= std::time::Duration::from_secs(30) {
                if write.send(Message::Ping(vec![])).await.is_err() {
                    app.connected = false;
                    app.add_message("[system] Connection lost. Reconnecting...".to_string());
                    app.scroll_to_bottom();
                    break 'chat; // back to reconnection loop
                }
                last_heartbeat = std::time::Instant::now();
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
                                        if !app.input.is_empty() && app.connected {
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
                                                    if write
                                                        .send(Message::Text(send_msg.to_string()))
                                                        .await
                                                        .is_err()
                                                    {
                                                        app.connected = false;
                                                        app.add_message("[system] Connection lost. Reconnecting...".to_string());
                                                        app.scroll_to_bottom();
                                                        break 'chat;
                                                    }
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
                                    KeyCode::Esc => break 'session,
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
                                KeyCode::Esc => break 'session,
                                _ => {}
                            },
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
