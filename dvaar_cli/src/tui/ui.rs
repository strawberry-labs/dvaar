//! TUI rendering functions

use super::app::{TuiApp, TunnelStatus, View};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table, TableState},
    Frame,
};

/// Draw the TUI
pub fn draw(frame: &mut Frame, app: &TuiApp) {
    match app.view {
        View::Main => draw_main_view(frame, app),
        View::RequestList => draw_request_list_view(frame, app),
    }
}

/// Draw the main view with tunnel info, metrics, and recent requests
fn draw_main_view(frame: &mut Frame, app: &TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),  // Title bar with ad
            Constraint::Length(9),  // Header with tunnel info + QR code
            Constraint::Length(1),  // Metrics row
            Constraint::Min(5),     // Request table
            Constraint::Length(1),  // Footer
        ])
        .split(frame.area());

    draw_title_bar(frame, app, chunks[0]);
    draw_header_with_qr(frame, app, chunks[1]);
    draw_metrics_row(frame, app, chunks[2]);
    draw_recent_requests(frame, app, chunks[3]);
    draw_footer(frame, chunks[4], false);
}

/// Draw the full request list view
fn draw_request_list_view(frame: &mut Frame, app: &TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(frame.area());

    draw_all_requests(frame, app, chunks[0]);
    draw_footer(frame, chunks[1], true);
}

/// Draw the title bar with Dvaar branding and rotating ad
fn draw_title_bar(frame: &mut Frame, app: &TuiApp, area: Rect) {
    let ad = app.current_ad();
    let ad_text = ad.map(|a| format!("Sponsored by {} - {} → {}", a.title, a.description, a.url))
        .unwrap_or_default();

    let lines = vec![
        Line::from(vec![
            Span::styled("  ╔═══════════════╗  ", Style::default().fg(Color::Cyan)),
            Span::styled(&ad_text, Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("  ║    ", Style::default().fg(Color::Cyan)),
            Span::styled("DVAAR", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("    ║  ", Style::default().fg(Color::Cyan)),
        ]),
    ];

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

/// Draw the header with tunnel info and QR code on the right
fn draw_header_with_qr(frame: &mut Frame, app: &TuiApp, area: Rect) {
    // Split header into left (info) and right (QR code)
    let qr_width = app.qr_code_lines.first().map(|l| l.chars().count()).unwrap_or(0) as u16 + 2;
    let qr_width = qr_width.min(30).max(15); // Reasonable bounds

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(40),           // Left: tunnel info
            Constraint::Length(qr_width),  // Right: QR code
        ])
        .split(area);

    draw_tunnel_info(frame, app, chunks[0]);
    draw_qr_code(frame, app, chunks[1]);
}

/// Draw tunnel info on the left side
fn draw_tunnel_info(frame: &mut Frame, app: &TuiApp, area: Rect) {
    let status_color = match app.tunnel_info.status {
        TunnelStatus::Online => Color::Green,
        TunnelStatus::Connecting => Color::Yellow,
        TunnelStatus::Reconnecting => Color::Yellow,
        TunnelStatus::Offline => Color::Red,
    };

    let latency_str = app
        .tunnel_info
        .latency_ms
        .map(|ms| format!("{}ms", ms))
        .unwrap_or_else(|| "-".to_string());

    let user_str = match (&app.tunnel_info.user_email, &app.tunnel_info.user_plan) {
        (Some(email), Some(plan)) => format!("{} ({})", email, plan),
        (Some(email), None) => email.clone(),
        (None, Some(plan)) => format!("Anonymous ({})", plan),
        (None, None) => "Anonymous".to_string(),
    };

    let inspector_str = app
        .tunnel_info
        .inspector_url
        .as_deref()
        .unwrap_or("disabled");

    // Truncate URLs to fit
    let max_url_len = (area.width as usize).saturating_sub(20);
    let public_url = truncate_str(&app.tunnel_info.public_url, max_url_len);
    let local_addr = truncate_str(&app.tunnel_info.local_addr, 25);
    let inspector_str = truncate_str(inspector_str, max_url_len);

    let lines = vec![
        Line::from(vec![
            Span::styled("Session Status  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                app.tunnel_info.status.as_str(),
                Style::default().fg(status_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Account         ", Style::default().fg(Color::DarkGray)),
            Span::styled(user_str, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Version         ", Style::default().fg(Color::DarkGray)),
            Span::styled(&app.tunnel_info.version, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Latency         ", Style::default().fg(Color::DarkGray)),
            Span::styled(latency_str, Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Forwarding      ", Style::default().fg(Color::DarkGray)),
            Span::styled(public_url, Style::default().fg(Color::Green)),
            Span::styled(" -> ", Style::default().fg(Color::DarkGray)),
            Span::styled(local_addr, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("Inspector       ", Style::default().fg(Color::DarkGray)),
            Span::styled(inspector_str, Style::default().fg(Color::Magenta)),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

/// Draw QR code on the right side
fn draw_qr_code(frame: &mut Frame, app: &TuiApp, area: Rect) {
    let qr_lines: Vec<Line> = app
        .qr_code_lines
        .iter()
        .take(area.height as usize)
        .map(|line| Line::from(Span::styled(line.as_str(), Style::default().fg(Color::White))))
        .collect();

    let block = Block::default()
        .borders(Borders::LEFT | Borders::BOTTOM)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(qr_lines).block(block);
    frame.render_widget(paragraph, area);
}

/// Draw the metrics row (compact, single line)
fn draw_metrics_row(frame: &mut Frame, app: &TuiApp, area: Rect) {
    let m = &app.metrics;

    let text = Line::from(vec![
        Span::styled("Conn ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{}↓ ", m.total_requests), Style::default().fg(Color::White)),
        Span::styled(format!("{}● ", m.open_connections), Style::default().fg(Color::Green)),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled("Rate ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{:.1}/m ", m.requests_per_minute_1m), Style::default().fg(Color::White)),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled("p50:", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{}ms ", m.p50_duration_ms), Style::default().fg(Color::White)),
        Span::styled("p90:", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{}ms", m.p90_duration_ms), Style::default().fg(Color::Yellow)),
    ]);

    let paragraph = Paragraph::new(text);
    frame.render_widget(paragraph, area);
}

/// Draw recent requests (last 10)
fn draw_recent_requests(frame: &mut Frame, app: &TuiApp, area: Rect) {
    // Calculate available width for path column
    // Fixed columns: Time(9) + Method(7) + Status(4) + Duration(8) + borders/padding(~10)
    let fixed_width = 9 + 7 + 4 + 8 + 10;
    let path_width = (area.width as usize).saturating_sub(fixed_width).max(10);

    let header = Row::new(vec!["Time", "Method", "Path", "Stat", "Time"])
        .style(Style::default().fg(Color::DarkGray))
        .bottom_margin(0);

    let rows: Vec<Row> = app
        .recent_requests
        .iter()
        .map(|req| {
            let method_style = method_style(&req.method);
            let status_style = status_style(req.response_status);

            Row::new(vec![
                Cell::from(format_timestamp(&req.timestamp)),
                Cell::from(format!("{:>6}", truncate_str(&req.method, 6))).style(method_style),
                Cell::from(truncate_path(&req.path, path_width)),
                Cell::from(req.response_status.to_string()).style(status_style),
                Cell::from(format_duration_short(req.duration_ms)),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(9),
            Constraint::Length(7),
            Constraint::Min(10),
            Constraint::Length(4),
            Constraint::Length(8),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(" HTTP Requests ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(table, area);
}

/// Draw all requests with scrolling and scrollbar
fn draw_all_requests(frame: &mut Frame, app: &TuiApp, area: Rect) {
    // Calculate available width for path column
    let fixed_width = 9 + 7 + 4 + 8 + 8 + 4; // time + method + status + duration + size + padding
    let path_width = (area.width as usize).saturating_sub(fixed_width).max(10);

    let header = Row::new(vec!["Time", "Method", "Path", "Stat", "Time", "Size"])
        .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .bottom_margin(0);

    let rows: Vec<Row> = app
        .all_requests
        .iter()
        .enumerate()
        .map(|(i, req)| {
            let method_style = method_style(&req.method);
            let status_style = status_style(req.response_status);
            let row_style = if i == app.selected_index {
                Style::default().bg(Color::Rgb(40, 40, 60))
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(format_timestamp(&req.timestamp)),
                Cell::from(format!("{:>6}", truncate_str(&req.method, 6))).style(method_style),
                Cell::from(truncate_path(&req.path, path_width)),
                Cell::from(req.response_status.to_string()).style(status_style),
                Cell::from(format_duration_short(req.duration_ms)),
                Cell::from(format_size_short(req.size_bytes)),
            ])
            .style(row_style)
        })
        .collect();

    // Split area to leave room for scrollbar
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let table = Table::new(
        rows,
        [
            Constraint::Length(9),
            Constraint::Length(7),
            Constraint::Min(10),
            Constraint::Length(4),
            Constraint::Length(8),
            Constraint::Length(8),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(format!(" All Requests ({}) ", app.all_requests.len()))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    )
    .row_highlight_style(Style::default().bg(Color::Rgb(40, 40, 60)));

    let mut state = TableState::default();
    state.select(Some(app.selected_index));

    frame.render_stateful_widget(table, chunks[0], &mut state);

    // Render scrollbar
    if !app.all_requests.is_empty() {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"))
            .track_symbol(Some("│"))
            .thumb_symbol("█");

        let mut scrollbar_state = ScrollbarState::new(app.all_requests.len())
            .position(app.selected_index);

        frame.render_stateful_widget(scrollbar, chunks[1], &mut scrollbar_state);
    }
}

/// Draw the footer with key hints
fn draw_footer(frame: &mut Frame, area: Rect, is_request_list: bool) {
    let text = if is_request_list {
        Line::from(vec![
            Span::styled("Esc", Style::default().fg(Color::Cyan)),
            Span::styled(" Back  ", Style::default().fg(Color::DarkGray)),
            Span::styled("↑/↓", Style::default().fg(Color::Cyan)),
            Span::styled(" Navigate  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Ctrl+C", Style::default().fg(Color::Cyan)),
            Span::styled(" Quit", Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(vec![
            Span::styled("Ctrl+O", Style::default().fg(Color::Cyan)),
            Span::styled(" View all requests  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Ctrl+C", Style::default().fg(Color::Cyan)),
            Span::styled(" Quit", Style::default().fg(Color::DarkGray)),
        ])
    };

    let paragraph = Paragraph::new(text);
    frame.render_widget(paragraph, area);
}

/// Get style for HTTP method
fn method_style(method: &str) -> Style {
    match method {
        "GET" => Style::default().fg(Color::Green),
        "POST" => Style::default().fg(Color::Yellow),
        "PUT" => Style::default().fg(Color::Blue),
        "PATCH" => Style::default().fg(Color::Magenta),
        "DELETE" => Style::default().fg(Color::Red),
        "HEAD" => Style::default().fg(Color::Cyan),
        "OPTIONS" => Style::default().fg(Color::White),
        _ => Style::default().fg(Color::White),
    }
}

/// Get style for HTTP status code
fn status_style(status: u16) -> Style {
    if status >= 500 {
        Style::default().fg(Color::Red)
    } else if status >= 400 {
        Style::default().fg(Color::Yellow)
    } else if status >= 300 {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::Green)
    }
}

/// Truncate path if too long
fn truncate_path(path: &str, max_len: usize) -> String {
    if path.len() > max_len {
        format!("{}...", &path[..max_len - 3])
    } else {
        path.to_string()
    }
}

/// Format timestamp for display
fn format_timestamp(timestamp: &chrono::DateTime<chrono::Utc>) -> String {
    timestamp.format("%H:%M:%S").to_string()
}

/// Format size in bytes
fn format_size(bytes: usize) -> String {
    if bytes >= 1_000_000 {
        format!("{:.1}MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.1}KB", bytes as f64 / 1_000.0)
    } else {
        format!("{}B", bytes)
    }
}

/// Format size in bytes (short version for tables)
fn format_size_short(bytes: usize) -> String {
    if bytes >= 1_000_000 {
        format!("{:.0}M", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.0}K", bytes as f64 / 1_000.0)
    } else {
        format!("{}B", bytes)
    }
}

/// Format duration in ms (short version for tables)
fn format_duration_short(ms: u64) -> String {
    if ms >= 1000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        format!("{}ms", ms)
    }
}

/// Truncate any string to max length
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() > max_len && max_len > 3 {
        format!("{}...", &s[..max_len - 3])
    } else if s.len() > max_len {
        s[..max_len].to_string()
    } else {
        s.to_string()
    }
}
