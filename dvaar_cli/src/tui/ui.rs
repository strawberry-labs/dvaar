//! TUI rendering functions

use super::app::{TuiApp, TunnelStatus, View};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table, TableState},
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
    // Calculate QR height to determine header height
    let qr_height = app.qr_code_lines.len().min(12) as u16;
    let header_height = qr_height.max(13) + 3; // +3 for borders, includes connections line

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_height),  // Header with all info including connections
            Constraint::Min(5),     // Request table
            Constraint::Length(1),  // Footer
        ])
        .split(frame.area());

    draw_unified_header(frame, app, chunks[0]);
    draw_recent_requests(frame, app, chunks[1]);
    draw_footer_simple(frame, chunks[2]);
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
    draw_footer_nav(frame, chunks[1]);
}

/// Draw unified header with tunnel info on left, QR code on right, all in one box
fn draw_unified_header(frame: &mut Frame, app: &TuiApp, area: Rect) {
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

    // Clear the area first to prevent content bleed-through
    frame.render_widget(Clear, area);

    // Create the outer block with explicit style to ensure interior is cleared
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .style(Style::default()); // Ensures interior is cleared

    frame.render_widget(block, area);

    // Inner area after borders
    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    // Calculate QR code dimensions
    let qr_natural_width = app.qr_code_lines.first().map(|l| l.chars().count()).unwrap_or(0) as u16;
    let show_qr = qr_natural_width > 0 && inner.width >= 60;

    let (info_area, qr_area) = if show_qr {
        let qr_width = qr_natural_width.min(inner.width / 3).min(30);
        let info_width = inner.width.saturating_sub(qr_width + 1);
        (
            Rect { x: inner.x, y: inner.y, width: info_width, height: inner.height },
            Some(Rect { x: inner.x + info_width, y: inner.y, width: qr_width + 1, height: inner.height }),
        )
    } else {
        (inner, None)
    };

    // Truncate URLs to fit info area
    let max_url_len = (info_area.width as usize).saturating_sub(14);
    let public_url = truncate_str(&app.tunnel_info.public_url, max_url_len);
    let local_addr = truncate_str(&app.tunnel_info.local_addr, 25);
    let inspector_str = truncate_str(inspector_str, max_url_len);

    // Sponsor line - get URL and description (URL for clickable link)
    let (sponsor_url, sponsor_desc) = app.current_ad()
        .map(|a| (a.url.clone(), a.description.clone()))
        .unwrap_or_default();

    // DVAAR logo using half-block characters (2 rows tall)
    let logo_style = Style::default().fg(Color::White);
    let version_style = Style::default().fg(Color::DarkGray);

    let info_lines = vec![
        // Logo row 1
        Line::from(vec![
            Span::styled("█▀▄ █ █ ▄▀█ ▄▀█ █▀█", logo_style),
            Span::styled(format!("  v{}", app.tunnel_info.version), version_style),
        ]),
        // Logo row 2
        Line::from(Span::styled("█▄▀ ▀▄▀ █▀█ █▀█ █▀▄", logo_style)),
        // Empty line after logo
        Line::from(""),
        // Sponsor line with "Sponsored by:" prefix, URL is underlined for Cmd+click
        Line::from(vec![
            Span::styled("Sponsored by: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&sponsor_url, Style::default().fg(Color::Yellow).add_modifier(Modifier::UNDERLINED)),
            Span::styled(" - ", Style::default().fg(Color::DarkGray)),
            Span::styled(&sponsor_desc, Style::default().fg(Color::Yellow)),
        ]),
        // Empty line after sponsor
        Line::from(""),
        // Status line
        Line::from(vec![
            Span::styled("Status      ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                app.tunnel_info.status.as_str(),
                Style::default().fg(status_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        // Latency line
        Line::from(vec![
            Span::styled("Latency     ", Style::default().fg(Color::DarkGray)),
            Span::styled(&latency_str, Style::default().fg(Color::White)),
        ]),
        // Account line
        Line::from(vec![
            Span::styled("Account     ", Style::default().fg(Color::DarkGray)),
            Span::styled(&user_str, Style::default().fg(Color::White)),
        ]),
        // Forwarding line with underlined links
        Line::from(vec![
            Span::styled("Forwarding  ", Style::default().fg(Color::DarkGray)),
            Span::styled(&public_url, Style::default().fg(Color::Green).add_modifier(Modifier::UNDERLINED)),
            Span::styled(" → ", Style::default().fg(Color::DarkGray)),
            Span::styled(&local_addr, Style::default().fg(Color::Cyan).add_modifier(Modifier::UNDERLINED)),
        ]),
        // Inspector line with underlined link
        Line::from(vec![
            Span::styled("Inspector   ", Style::default().fg(Color::DarkGray)),
            Span::styled(&inspector_str, Style::default().fg(Color::Magenta).add_modifier(Modifier::UNDERLINED)),
        ]),
        // Empty line before connections
        Line::from(""),
        // Connections line
        {
            let m = &app.metrics;
            Line::from(vec![
                Span::styled("Connections ", Style::default().fg(Color::DarkGray)),
                Span::styled("ttl ", Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{:<4}", m.total_requests), Style::default().fg(Color::White)),
                Span::styled(" opn ", Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{:<4}", m.open_connections), Style::default().fg(Color::Green)),
                Span::styled(" rt1 ", Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{:<6.2}", m.requests_per_minute_1m), Style::default().fg(Color::White)),
                Span::styled(" rt5 ", Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{:<6.2}", m.requests_per_minute_5m), Style::default().fg(Color::White)),
            ])
        },
    ];

    // Clear info area and render paragraph
    frame.render_widget(Clear, info_area);
    let info_paragraph = Paragraph::new(info_lines);
    frame.render_widget(info_paragraph, info_area);

    // Draw QR code if space available
    if let Some(qr_rect) = qr_area {
        frame.render_widget(Clear, qr_rect);
        let available_height = qr_rect.height as usize;
        let available_width = qr_rect.width.saturating_sub(1) as usize;
        let qr_natural_width = app.qr_code_lines.first().map(|l| l.chars().count()).unwrap_or(0);

        // Calculate scale factor
        let scale = if qr_natural_width <= available_width {
            1
        } else if qr_natural_width <= available_width * 2 {
            2
        } else {
            3
        };

        let qr_lines: Vec<Line> = app
            .qr_code_lines
            .iter()
            .step_by(scale)
            .take(available_height)
            .map(|line| {
                let scaled: String = line.chars()
                    .enumerate()
                    .filter(|(i, _)| i % scale == 0)
                    .map(|(_, c)| c)
                    .take(available_width)
                    .collect();
                Line::from(Span::styled(scaled, Style::default().fg(Color::White).bg(Color::Black)))
            })
            .collect();

        let qr_paragraph = Paragraph::new(qr_lines);
        frame.render_widget(qr_paragraph, Rect {
            x: qr_rect.x + 1,
            y: qr_rect.y,
            width: qr_rect.width.saturating_sub(1),
            height: qr_rect.height,
        });
    }
}

/// Draw recent requests (last 10) - responsive layout
fn draw_recent_requests(frame: &mut Frame, app: &TuiApp, area: Rect) {
    let width = area.width as usize;

    // Responsive column layout based on terminal width
    // Order: Status + Time + Duration + Method + Path (Path is always last)
    let (header_cells, constraints, row_builder): (Vec<&str>, Vec<Constraint>, Box<dyn Fn(&crate::inspector::CapturedRequest, usize) -> Vec<Cell>>) =
        if width >= 80 {
            // Full layout: Status + Time + Duration + Method + Path
            let path_width = width.saturating_sub(6 + 9 + 10 + 7 + 10).max(10);
            (
                vec!["Status", "Time", "Duration", "Method", "Path"],
                vec![
                    Constraint::Length(6),
                    Constraint::Length(9),
                    Constraint::Length(10),
                    Constraint::Length(7),
                    Constraint::Min(10),
                ],
                Box::new(move |req, _| vec![
                    Cell::from(format!("{:>3}", req.response_status)).style(status_style(req.response_status)),
                    Cell::from(format_datetime(&req.timestamp)),
                    Cell::from(format_duration_short(req.duration_ms)),
                    Cell::from(format!("{:>6}", truncate_str(&req.method, 6))).style(method_style(&req.method)),
                    Cell::from(truncate_path(&req.path, path_width)),
                ])
            )
        } else if width >= 50 {
            // Compact layout: Status + Time + Method + Path
            let path_width = width.saturating_sub(6 + 9 + 7 + 6).max(10);
            (
                vec!["Status", "Time", "Method", "Path"],
                vec![
                    Constraint::Length(6),
                    Constraint::Length(9),
                    Constraint::Length(7),
                    Constraint::Min(10),
                ],
                Box::new(move |req, _| vec![
                    Cell::from(format!("{:>3}", req.response_status)).style(status_style(req.response_status)),
                    Cell::from(format_timestamp(&req.timestamp)),
                    Cell::from(format!("{:>6}", truncate_str(&req.method, 6))).style(method_style(&req.method)),
                    Cell::from(truncate_path(&req.path, path_width)),
                ])
            )
        } else {
            // Minimal layout: Status + Method + Path only
            let path_width = width.saturating_sub(6 + 7 + 4).max(5);
            (
                vec!["Status", "Method", "Path"],
                vec![
                    Constraint::Length(6),
                    Constraint::Length(7),
                    Constraint::Min(5),
                ],
                Box::new(move |req, _| vec![
                    Cell::from(format!("{:>3}", req.response_status)).style(status_style(req.response_status)),
                    Cell::from(format!("{:>6}", truncate_str(&req.method, 6))).style(method_style(&req.method)),
                    Cell::from(truncate_path(&req.path, path_width)),
                ])
            )
        };

    let header = Row::new(header_cells)
        .style(Style::default().fg(Color::DarkGray))
        .bottom_margin(0);

    let rows: Vec<Row> = app
        .recent_requests
        .iter()
        .enumerate()
        .map(|(i, req)| Row::new(row_builder(req, i)))
        .collect();

    let table = Table::new(rows, constraints)
        .header(header)
        .block(
            Block::default()
                .title(" HTTP Requests ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .style(Style::default()), // Ensures interior is cleared
        );

    // Clear area first to prevent bleed-through
    frame.render_widget(Clear, area);
    frame.render_widget(table, area);
}

/// Draw all requests with scrolling and scrollbar
fn draw_all_requests(frame: &mut Frame, app: &TuiApp, area: Rect) {
    // Calculate available width for path column
    // Order: Status + Time + Duration + Method + Path + Size
    let fixed_width = 6 + 9 + 10 + 7 + 8 + 6; // status + time + duration + method + size + padding
    let path_width = (area.width as usize).saturating_sub(fixed_width).max(10);

    let header = Row::new(vec!["Status", "Time", "Duration", "Method", "Path", "Size"])
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
                Cell::from(format!("{:>3}", req.response_status)).style(status_style),
                Cell::from(format_datetime(&req.timestamp)),
                Cell::from(format_duration_short(req.duration_ms)),
                Cell::from(format!("{:>6}", truncate_str(&req.method, 6))).style(method_style),
                Cell::from(truncate_path(&req.path, path_width)),
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
            Constraint::Length(6),
            Constraint::Length(9),
            Constraint::Length(10),
            Constraint::Length(7),
            Constraint::Min(10),
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

/// Draw footer with key hints for main view
fn draw_footer_simple(frame: &mut Frame, area: Rect) {
    let text = Line::from(vec![
        Span::styled("[", Style::default().fg(Color::DarkGray)),
        Span::styled("Ctrl+O", Style::default().fg(Color::Cyan)),
        Span::styled("] View requests  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[", Style::default().fg(Color::DarkGray)),
        Span::styled("Ctrl+C", Style::default().fg(Color::Cyan)),
        Span::styled("] Quit", Style::default().fg(Color::DarkGray)),
    ]);

    let paragraph = Paragraph::new(text);
    frame.render_widget(paragraph, area);
}

/// Draw footer with navigation hints for request list view
fn draw_footer_nav(frame: &mut Frame, area: Rect) {
    let text = Line::from(vec![
        Span::styled("[", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::Cyan)),
        Span::styled("] Back  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[", Style::default().fg(Color::DarkGray)),
        Span::styled("↑/↓", Style::default().fg(Color::Cyan)),
        Span::styled("] Navigate  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[", Style::default().fg(Color::DarkGray)),
        Span::styled("Ctrl+C", Style::default().fg(Color::Cyan)),
        Span::styled("] Quit", Style::default().fg(Color::DarkGray)),
    ]);

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

/// Format timestamp for display (time only)
fn format_timestamp(timestamp: &chrono::DateTime<chrono::Utc>) -> String {
    timestamp.format("%H:%M:%S").to_string()
}

/// Format datetime for display (time only, local timezone)
fn format_datetime(timestamp: &chrono::DateTime<chrono::Utc>) -> String {
    use chrono::Local;
    timestamp.with_timezone(&Local).format("%H:%M:%S").to_string()
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
