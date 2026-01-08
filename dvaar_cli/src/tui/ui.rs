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
            Constraint::Length(9),  // Header with title + sponsor + tunnel info + QR code
            Constraint::Length(1),  // Metrics row
            Constraint::Min(5),     // Request table
            Constraint::Length(1),  // Footer
        ])
        .split(frame.area());

    draw_header_with_qr(frame, app, chunks[0]);
    draw_metrics_row(frame, app, chunks[1]);
    draw_recent_requests(frame, app, chunks[2]);
    draw_footer_simple(frame, chunks[3]);
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

/// Draw the header with tunnel info and QR code on the right (responsive sizing)
fn draw_header_with_qr(frame: &mut Frame, app: &TuiApp, area: Rect) {
    let qr_natural_width = app.qr_code_lines.first().map(|l| l.chars().count()).unwrap_or(0) as u16;

    // Don't show QR if empty or terminal is very narrow
    if qr_natural_width == 0 || area.width < 60 {
        draw_tunnel_info(frame, app, area);
        return;
    }

    // Calculate available space for QR code (reserve at least 45 cols for tunnel info)
    let min_info_width = 45u16;
    let available_for_qr = area.width.saturating_sub(min_info_width);

    // Scale QR code to fit: use full size, or shrink if needed
    let qr_width = (qr_natural_width + 2).min(available_for_qr).min(35);

    if qr_width < 15 {
        // Too narrow for a useful QR code
        draw_tunnel_info(frame, app, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(min_info_width),  // Left: tunnel info
            Constraint::Length(qr_width),      // Right: QR code (scaled)
        ])
        .split(area);

    draw_tunnel_info(frame, app, chunks[0]);
    draw_qr_code_scaled(frame, app, chunks[1]);
}

/// Draw tunnel info on the left side with title
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
    let max_url_len = (area.width as usize).saturating_sub(18);
    let public_url = truncate_str(&app.tunnel_info.public_url, max_url_len);
    let local_addr = truncate_str(&app.tunnel_info.local_addr, 25);
    let inspector_str = truncate_str(inspector_str, max_url_len);

    // Sponsor line text - show full sponsor info
    let sponsor_text = app.current_ad()
        .map(|a| format!("{} - {}", a.title, a.description))
        .unwrap_or_default();

    let lines = vec![
        // Title line: dvaar (bold white) + version (grey in brackets)
        Line::from(vec![
            Span::styled("dvaar ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(format!("(v{})", app.tunnel_info.version), Style::default().fg(Color::DarkGray)),
        ]),
        // Empty line between version and sponsor
        Line::from(""),
        // Sponsor line (below title, above status) in yellow
        Line::from(Span::styled(&sponsor_text, Style::default().fg(Color::Yellow))),
        Line::from(vec![
            Span::styled("Status    ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                app.tunnel_info.status.as_str(),
                Style::default().fg(status_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  │  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Latency ", Style::default().fg(Color::DarkGray)),
            Span::styled(&latency_str, Style::default().fg(Color::White)),
            Span::styled("  │  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Account ", Style::default().fg(Color::DarkGray)),
            Span::styled(&user_str, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Forwarding  ", Style::default().fg(Color::DarkGray)),
            Span::styled(&public_url, Style::default().fg(Color::Green)),
            Span::styled(" → ", Style::default().fg(Color::DarkGray)),
            Span::styled(&local_addr, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("Inspector   ", Style::default().fg(Color::DarkGray)),
            Span::styled(&inspector_str, Style::default().fg(Color::Magenta)),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

/// Draw QR code on the right side, scaled to fit available space
fn draw_qr_code_scaled(frame: &mut Frame, app: &TuiApp, area: Rect) {
    // Available space for QR (accounting for border)
    let available_width = area.width.saturating_sub(2) as usize;
    let available_height = area.height.saturating_sub(1) as usize;

    let qr_natural_width = app.qr_code_lines.first().map(|l| l.chars().count()).unwrap_or(0);

    // Calculate scale factor (1 = full size, 2 = half size, etc.)
    let scale = if qr_natural_width <= available_width {
        1 // Full size fits
    } else if qr_natural_width <= available_width * 2 {
        2 // Half size
    } else {
        3 // Third size (minimum)
    };

    let qr_lines: Vec<Line> = app
        .qr_code_lines
        .iter()
        .step_by(scale) // Skip rows to scale vertically
        .take(available_height)
        .map(|line| {
            // Scale horizontally by taking every Nth character
            let scaled: String = line.chars()
                .enumerate()
                .filter(|(i, _)| i % scale == 0)
                .map(|(_, c)| c)
                .take(available_width)
                .collect();
            Line::from(Span::styled(scaled, Style::default().fg(Color::White).bg(Color::Black)))
        })
        .collect();

    let block = Block::default()
        .borders(Borders::LEFT | Borders::BOTTOM)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(qr_lines).block(block);
    frame.render_widget(paragraph, area);
}

/// Draw the metrics row (compact, responsive) - format: Connections ttl opn rt1 rt5 p50 p90
fn draw_metrics_row(frame: &mut Frame, app: &TuiApp, area: Rect) {
    let m = &app.metrics;
    let width = area.width as usize;

    // Format like: Connections     ttl    opn    rt1    rt5    p50    p90
    //                              1      0      0.00   0.00   148.60 148.60
    let mut spans = vec![
        Span::styled("Connections", Style::default().fg(Color::DarkGray)),
    ];

    // Always show ttl and opn
    spans.extend([
        Span::styled("  ttl ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{:<4}", m.total_requests), Style::default().fg(Color::White)),
        Span::styled("opn ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{:<4}", m.open_connections), Style::default().fg(Color::Green)),
    ]);

    // Add rate columns if we have space (width > 50)
    if width > 50 {
        spans.extend([
            Span::styled("rt1 ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:<6.2}", m.requests_per_minute_1m), Style::default().fg(Color::White)),
            Span::styled("rt5 ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:<6.2}", m.requests_per_minute_5m), Style::default().fg(Color::White)),
        ]);
    }

    // Add percentiles if we have more space (width > 80)
    if width > 80 {
        spans.extend([
            Span::styled("p50 ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:<6.2}", m.p50_duration_ms as f64), Style::default().fg(Color::White)),
            Span::styled("p90 ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:<6.2}", m.p90_duration_ms as f64), Style::default().fg(Color::Yellow)),
        ]);
    }

    let paragraph = Paragraph::new(Line::from(spans));
    frame.render_widget(paragraph, area);
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
                .border_style(Style::default().fg(Color::DarkGray)),
        );

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
