//! Ratatui layout and rendering for the dashboard and add-upstream modal.

use super::app::{AddField, AddUpstreamForm, App};
use comux_core::mask_token;
use comux_core::RedirectPolicy;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

/// Renders the complete dashboard and optional add-upstream modal.
pub fn render(frame: &mut Frame<'_>, app: &App) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(4),
        ])
        .split(frame.area());
    render_header(frame, areas[0], app);
    render_body(frame, areas[1], app);
    render_footer(frame, areas[2], app);
    if let Some(form) = &app.add_form {
        render_add_form(frame, frame.area(), form);
    }
}

/// Renders the title, active upstream, and current status message.
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

/// Splits the main area into upstream and usage panels.
fn render_body(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(area);
    render_upstreams(frame, rows[0], app);
    render_stats(frame, rows[1], app);
}

/// Renders upstream rows with selection and token masking.
fn render_upstreams(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let mut lines = Vec::new();
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

/// Renders compact process-local usage counters by upstream.
fn render_stats(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let mut lines = Vec::new();
    match &app.stats {
        Some(stats) if stats.by_upstream.is_empty() => lines.push(Line::raw("  no requests")),
        Some(stats) => {
            for (name, value) in &stats.by_upstream {
                lines.push(Line::raw(format!(
                    "{name}: {} req, {} active, {} -> {}",
                    value.accepted_requests,
                    value.active_streams,
                    format_bytes(value.request_bytes),
                    format_bytes(value.response_bytes),
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

/// Renders the dashboard keyboard shortcut reference.
fn render_footer(frame: &mut Frame<'_>, area: Rect, _app: &App) {
    let text = Text::from(vec![
        Line::raw(" j/k or arrows: move   Enter: select   a: add   d: delete "),
        Line::raw(" r: refresh   t: regenerate client token   q/Esc: quit "),
    ]);
    frame.render_widget(
        Paragraph::new(text).block(Block::default().borders(Borders::ALL)),
        area,
    );
}

/// Renders the centered add-upstream form and any validation error.
fn render_add_form(frame: &mut Frame<'_>, area: Rect, form: &AddUpstreamForm) {
    let width = area.width.saturating_sub(4).min(70);
    let height = area.height.saturating_sub(4).min(14);
    let modal = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    };
    let value = |field: AddField, text: &str| {
        let marker = if form.field == field { ">" } else { " " };
        let display = if field == AddField::AuthToken && !text.is_empty() {
            "*".repeat(text.chars().count())
        } else if text.is_empty() {
            "_".to_owned()
        } else {
            text.to_owned()
        };
        Line::from(vec![
            Span::styled(
                format!("{marker} "),
                Style::default().fg(if form.field == field {
                    Color::Yellow
                } else {
                    Color::Reset
                }),
            ),
            Span::raw(format!("{}: {display}", field_name(field))),
        ])
    };
    let policy_marker = if form.field == AddField::RedirectPolicy {
        ">"
    } else {
        " "
    };
    let policy = Line::from(vec![
        Span::styled(
            format!("{policy_marker} "),
            Style::default().fg(if form.field == AddField::RedirectPolicy {
                Color::Yellow
            } else {
                Color::Reset
            }),
        ),
        Span::raw(format!(
            "Redirect policy: {}",
            policy_name(&form.redirect_policy)
        )),
    ]);
    let mut lines = vec![
        value(AddField::Name, &form.name),
        value(AddField::BaseUrl, &form.base_url),
        value(AddField::AuthToken, &form.auth_token),
        policy,
        Line::raw(""),
        Line::raw("Enter: next/submit   Esc: cancel   Left/Right: policy"),
    ];
    if let Some(error) = &form.error {
        lines.insert(4, Line::styled(error, Style::default().fg(Color::Red)));
    }
    frame.render_widget(Clear, modal);
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Add Upstream "),
            )
            .wrap(Wrap { trim: true }),
        modal,
    );
}

/// Returns the human-readable label for an add-form field.
fn field_name(field: AddField) -> &'static str {
    match field {
        AddField::Name => "Name",
        AddField::BaseUrl => "Base URL",
        AddField::AuthToken => "Auth token",
        AddField::RedirectPolicy => "Redirect policy",
    }
}

/// Returns the serialized spelling of a redirect policy.
fn policy_name(policy: &RedirectPolicy) -> &'static str {
    match policy {
        RedirectPolicy::Always => "always",
        RedirectPolicy::Default => "default",
        RedirectPolicy::Never => "never",
    }
}

/// Formats a byte count with the smallest practical binary unit.
///
/// Byte values are integral. Larger units use no decimal when exact and one
/// decimal place otherwise.
pub fn format_bytes(bytes: u64) -> String {
    /// Binary unit labels indexed by repeated division by 1024.
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else if value.fract() == 0.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::{format_bytes, render};
    use crate::tui::app::App;
    use crate::tui::events::Action;
    use comux_core::api::{StatsResponse, TransportStats};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::collections::BTreeMap;

    fn screen(app: &App) -> String {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, app)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn formats_bytes_using_binary_units() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1024), "1 KiB");
        assert_eq!(format_bytes(1536), "1.5 KiB");
        assert_eq!(format_bytes(1024 * 1024), "1 MiB");
        assert_eq!(format_bytes(1024_u64.pow(3)), "1 GiB");
        assert_eq!(format_bytes(1024_u64.pow(4)), "1 TiB");
    }

    #[test]
    fn dashboard_uses_vertical_panels_without_inner_headings_and_has_two_footer_rows() {
        let mut app = App::placeholder();
        app.stats = Some(StatsResponse {
            by_upstream: BTreeMap::from([(
                "openai".to_owned(),
                TransportStats {
                    request_bytes: 1024,
                    response_bytes: 1024 * 1024,
                    ..TransportStats::default()
                },
            )]),
        });
        let text = screen(&app);
        assert!(text.contains("Upstreams"));
        assert!(text.contains("Usage"));
        assert!(!text.contains("UPSTREAMS"));
        assert!(!text.contains("STATS"));
        assert!(text.contains("1 KiB"));
        assert!(text.contains("1 MiB"));
        assert!(text.contains("a: add"));
        assert!(text.contains("r: refresh"));
    }

    #[tokio::test]
    async fn add_form_renders_as_a_labeled_modal() {
        let mut app = App::placeholder();
        app.handle(Action::Add).await;
        let text = screen(&app);
        assert!(text.contains("Add Upstream"));
        assert!(text.contains("Name"));
        assert!(text.contains("Base URL"));
        assert!(text.contains("Auth token"));
        assert!(text.contains("Redirect policy"));
    }
}
