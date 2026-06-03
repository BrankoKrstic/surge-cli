use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::cli::app::{App, Screen};

pub fn update(app: &mut App, key_event: KeyEvent) {
    match (key_event.code, app.screen) {
        (KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q'), Screen::Main) => {
            app.screen = Screen::Quit
        }
        (KeyCode::Char('y') | KeyCode::Char('Y'), Screen::Quit) => app.quit(),
        (KeyCode::Char('n') | KeyCode::Char('N'), Screen::Quit) => app.screen = Screen::Main,
        (KeyCode::Esc, _) => app.screen = Screen::Main,
        (KeyCode::Char('m') | KeyCode::Char('M'), Screen::Main) => app.toggle_mute(),
        (KeyCode::Char('+'), Screen::Main) => app.volume_up(),
        (KeyCode::Char('-'), Screen::Main) => app.volume_down(),

        (KeyCode::Char('c') | KeyCode::Char('C'), _)
            if key_event.modifiers == KeyModifiers::CONTROL =>
        {
            app.quit();
        }
        (KeyCode::Char('f') | KeyCode::Char('F'), Screen::Main) => app.screen = Screen::Search,
        (KeyCode::Char(x), Screen::Search) => app.push_char(x),
        (KeyCode::Backspace, Screen::Search) => app.pop_char(),
        (KeyCode::Up, Screen::Search) => app.move_cursor(super::app::Direction::Up),
        (KeyCode::Down, Screen::Search) => app.move_cursor(super::app::Direction::Down),
        (KeyCode::Enter, Screen::Search) => app.change_station(),

        _ => {}
    }
}
