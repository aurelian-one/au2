use std::io;
use std::io::{Result, stdout};

use crossterm::{event::{self}, ExecutableCommand, execute, terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen,
    LeaveAlternateScreen,
}};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event};
use ratatui::backend::Backend;
use ratatui::prelude::{CrosstermBackend, Stylize, Terminal};

use crate::app::App;
use crate::ui::ui;

mod app;
mod ui;

fn main() -> Result<()> {
    // Enter raw mode so that we no longer care about wrapping and backspaces and that sort
    // of thing.
    enable_raw_mode()?;
    // Enter a fresh new screen and start polling for mouse events.
    execute!(stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    // Set up Ratatui with the cross term backend
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let mut app = App::new();
    let _ = run_app(&mut terminal, &mut app);

    // Restore terminal back to original modes.
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    // Return.
    Ok(())
}

fn run_app<B: Backend>(term: &mut Terminal<B>, app: &mut App) -> io::Result<bool> {
    loop {
        term.draw(|f| ui(f, app))?;
        if let Event::Key(key) = event::read()? {
            continue
        }
    }
}