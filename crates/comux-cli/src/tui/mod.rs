//! Interactive terminal dashboard for monitoring and managing a proxy.

/// Dashboard state, refreshes, mutations, and add-upstream form handling.
pub mod app;
/// Conversion of keyboard events into dashboard actions.
pub mod events;
/// Ratatui layout and rendering helpers.
pub mod ui;

use crate::args::CliArgs;
use crate::client::ApiClient;
use crate::config::{default_config_path, resolve_config_interactive};
use app::App;
use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;
use std::time::{Duration, Instant};

/// Opens the terminal dashboard using resolved management settings.
///
/// This performs an initial refresh, enters raw alternate-screen mode, runs
/// the event loop, and restores the terminal before returning its result.
pub async fn run(args: CliArgs) -> Result<(), Box<dyn std::error::Error>> {
    let path = default_config_path();
    let config =
        resolve_config_interactive(&path, args.endpoint.clone(), args.admin_token.clone())?;
    let client = ApiClient::new(&config.admin_endpoint, config.admin_token)?;
    let mut app = App::new(client, args.dangerously_print_full_token);
    app.refresh().await;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    // Capture the loop result before cleanup so terminal state is restored
    // before a success or error is returned to the caller.
    let result = run_loop(&mut terminal, &mut app).await;
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

/// Redraws, handles keyboard input, and refreshes remote state every two seconds.
async fn run_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<(), Box<dyn std::error::Error>>
where
    B::Error: 'static,
{
    let mut next_refresh = Instant::now();
    loop {
        terminal.draw(|frame| ui::render(frame, app))?;
        let timeout = next_refresh
            .checked_duration_since(Instant::now())
            .unwrap_or(Duration::ZERO);
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if app.handle_key(key).await {
                    break;
                }
            }
        } else {
            app.refresh().await;
            next_refresh = Instant::now() + Duration::from_secs(2);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::app::App;
    use super::ui;
    use comux_core::api::{StatsResponse, StatusResponse};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn renders_non_blank_dashboard() {
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::placeholder();
        app.status = Some(StatusResponse {
            active_upstream: "openai".into(),
            downstream_bind: "127.0.0.1".into(),
            downstream_port: 6608,
            admin_bind: "127.0.0.1".into(),
            admin_port: 6609,
            upstream_names: vec!["openai".into()],
        });
        app.stats = Some(StatsResponse::default());
        terminal.draw(|frame| ui::render(frame, &app)).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(text.contains("comux"));
        assert!(text.contains("openai"));
    }
}
