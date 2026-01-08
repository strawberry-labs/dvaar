//! TUI rendering functions

use super::app::{TuiApp, TunnelStatus, View};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
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
            Constraint::Length(9),  // Header with tunnel info
            Constraint::Length(3),  // Metrics row
            Constraint::Min(5),     // Request table
            Constraint::Length(1),  // Footer
        ])
        .split(frame.area());

    draw_header(frame, app, chunks[0]);
    draw_metrics_row(frame, app, chunks[1]);
    draw_recent_requests(frame, app, chunks[2]);
    draw_footer(frame, chunks[3], false);
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

/// Draw the header with tunnel info
fn draw_header(frame: &mut Frame, app: &TuiApp, area: Rect) {
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

    let user_str = app
        .tunnel_info
        .user_email
        .as_deref()
        .unwrap_or("Anonymous");

    let inspector_str = app
        .tunnel_info
        .inspector_url
        .as_deref()
        .unwrap_or("disabled");

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
            Span::styled(&app.tunnel_info.public_url, Style::default().fg(Color::Green)),
            Span::styled(" -> ", Style::default().fg(Color::DarkGray)),
            Span::styled(&app.tunnel_info.local_addr, Style::default().fg(Color::Cyan)),
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

/// Draw the metrics row
fn draw_metrics_row(frame: &mut Frame, app: &TuiApp, area: Rect) {
    let m = &app.metrics;

    let text = Line::from(vec![
        Span::styled("Connections ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("ttl:{} ", m.total_requests),
            Style::default().fg(Color::White),
        ),
        Span::styled(
            format!("opn:{}", m.open_connections),
            Style::default().fg(Color::White),
        ),
        Span::styled("  |  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Rate ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("rt1:{:.2}/m ", m.requests_per_minute_1m),
            Style::default().fg(Color::White),
        ),
        Span::styled(
            format!("rt5:{:.2}/m", m.requests_per_minute_5m),
            Style::default().fg(Color::White),
        ),
        Span::styled("  |  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Latency ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("p50:{}ms ", m.p50_duration_ms),
            Style::default().fg(Color::White),
        ),
        Span::styled(
            format!("p90:{}ms", m.p90_duration_ms),
            Style::default().fg(Color::White),
        ),
    ]);

    let paragraph = Paragraph::new(text);
    frame.render_widget(paragraph, area);
}

/// Draw recent requests (last 10)
fn draw_recent_requests(frame: &mut Frame, app: &TuiApp, area: Rect) {
    let header = Row::new(vec!["Time", "Method", "Path", "Status", "Duration"])
        .style(Style::default().fg(Color::DarkGray))
        .bottom_margin(1);

    let rows: Vec<Row> = app
        .recent_requests
        .iter()
        .map(|req| {
            let method_style = method_style(&req.method);
            let status_style = status_style(req.response_status);

            Row::new(vec![
                Cell::from(format_timestamp(&req.timestamp)),
                Cell::from(format!("{:>7}", req.method)).style(method_style),
                Cell::from(truncate_path(&req.path, 50)),
                Cell::from(req.response_status.to_string()).style(status_style),
                Cell::from(format!("{}ms", req.duration_ms)),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Min(20),
            Constraint::Length(6),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title("HTTP Requests")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(table, area);
}

/// Draw all requests with scrolling
fn draw_all_requests(frame: &mut Frame, app: &TuiApp, area: Rect) {
    let header = Row::new(vec!["Time", "Method", "Path", "Status", "Duration", "Size"])
        .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .bottom_margin(1);

    let rows: Vec<Row> = app
        .all_requests
        .iter()
        .enumerate()
        .map(|(i, req)| {
            let method_style = method_style(&req.method);
            let status_style = status_style(req.response_status);
            let row_style = if i == app.selected_index {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(format_timestamp(&req.timestamp)),
                Cell::from(format!("{:>7}", req.method)).style(method_style),
                Cell::from(truncate_path(&req.path, 60)),
                Cell::from(req.response_status.to_string()).style(status_style),
                Cell::from(format!("{}ms", req.duration_ms)),
                Cell::from(format_size(req.size_bytes)),
            ])
            .style(row_style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Min(30),
            Constraint::Length(6),
            Constraint::Length(10),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(format!("All Requests ({} total)", app.all_requests.len()))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    )
    .row_highlight_style(Style::default().bg(Color::DarkGray));

    // Calculate scroll offset to keep selected item visible
    let _visible_rows = area.height.saturating_sub(4) as usize;
    let mut state = TableState::default();
    state.select(Some(app.selected_index));

    frame.render_stateful_widget(table, area, &mut state);
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
