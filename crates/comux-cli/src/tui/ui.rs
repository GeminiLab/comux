use super::app::App;
use comux_core::mask_token;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

pub fn render(frame: &mut Frame<'_>, app: &App) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(frame.area());
    render_header(frame, areas[0], app);
    render_body(frame, areas[1], app);
    render_footer(frame, areas[2], app);
}

fn render_header(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let active = app
        .status
        .as_ref()
        .map(|status| status.active_upstream.as_str())
        .unwrap_or("unknown");
    let text = Line::from(vec![
        Span::styled(" comux ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("Codex proxy  |  active: "),
        Span::styled(active, Style::default().fg(Color::Green)),
        Span::raw("  |  "),
        Span::raw(&app.message),
    ]);
    frame.render_widget(
        Paragraph::new(text).block(Block::default().borders(Borders::ALL)),
        area,
    );
}

fn render_body(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(area);
    render_upstreams(frame, columns[0], app);
    render_stats(frame, columns[1], app);
}

fn render_upstreams(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let mut lines = vec![Line::styled(
        "UPSTREAMS",
        Style::default().add_modifier(Modifier::BOLD),
    )];
    if app.upstreams.is_empty() {
        lines.push(Line::raw("  (none)"));
    }
    for (index, upstream) in app.upstreams.iter().enumerate() {
        let marker = if index == app.selected { ">" } else { " " };
        let token = if app.show_full_tokens {
            upstream.auth_token.as_str()
        } else {
            &mask_token(&upstream.auth_token)
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("{marker} {} ", upstream.name),
                if index == app.selected {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                },
            ),
            Span::raw(format!("{} [{}]", upstream.base_url, token)),
        ]));
    }
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(Block::default().borders(Borders::ALL).title(" Upstreams "))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_stats(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let mut lines = vec![Line::styled(
        "STATS",
        Style::default().add_modifier(Modifier::BOLD),
    )];
    match &app.stats {
        Some(stats) if stats.by_upstream.is_empty() => lines.push(Line::raw("  no requests")),
        Some(stats) => {
            for (name, value) in &stats.by_upstream {
                lines.push(Line::raw(format!(
                    "{name}: {} req, {} active, {} B -> {} B",
                    value.accepted_requests,
                    value.active_streams,
                    value.request_bytes,
                    value.response_bytes
                )));
            }
        }
        None => lines.push(Line::raw("  unavailable")),
    }
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(Block::default().borders(Borders::ALL).title(" Usage "))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, _app: &App) {
    let text = " j/k or arrows: move   Enter: select   d: delete   r: refresh   t: regenerate client token   q/Esc: quit ";
    frame.render_widget(
        Paragraph::new(text).block(Block::default().borders(Borders::ALL)),
        area,
    );
}
