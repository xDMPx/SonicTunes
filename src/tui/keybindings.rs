use crate::tui::commands::{TuiCommand, TuiState};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub struct Keybindings {
    pub map: std::collections::HashMap<KeyEvent, (TuiCommand, Option<&'static str>)>,
}

impl Keybindings {
    pub fn new() -> Self {
        let keybindings = std::collections::HashMap::from([
            (
                KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE),
                (TuiCommand::State(TuiState::Player), Some("view player")),
            ),
            (
                KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE),
                (TuiCommand::State(TuiState::History), Some("view history")),
            ),
            (
                KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE),
                (TuiCommand::State(TuiState::Help), Some("view help")),
            ),
            (
                KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
                (TuiCommand::Quit, Some("quit, q")),
            ),
            (
                KeyEvent::new(KeyCode::Char('{'), KeyModifiers::NONE),
                (TuiCommand::Volume(-1), Some("vol -1")),
            ),
            (
                KeyEvent::new(KeyCode::Char('}'), KeyModifiers::NONE),
                (TuiCommand::Volume(1), Some("vol +1")),
            ),
            (
                KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE),
                (TuiCommand::Volume(-10), Some("vol -10")),
            ),
            (
                KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE),
                (TuiCommand::Volume(10), Some("vol +10")),
            ),
            (
                KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
                (TuiCommand::Seek(-10.0), Some("seek -10")),
            ),
            (
                KeyEvent::new(KeyCode::Left, KeyModifiers::SHIFT),
                (TuiCommand::Seek(-60.0), Some("seek -60")),
            ),
            (
                KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
                (TuiCommand::Seek(10.0), Some("seek +10")),
            ),
            (
                KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT),
                (TuiCommand::Seek(60.0), Some("seek -60")),
            ),
            (
                KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
                (TuiCommand::PlayPause, Some("play-pause")),
            ),
            (
                KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE),
                (TuiCommand::PlayPrevious, Some("play-prev")),
            ),
            (
                KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
                (TuiCommand::PlayNext, Some("play-next")),
            ),
            (
                KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
                (TuiCommand::Scroll(1), Some("scroll +1")),
            ),
            (
                KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
                (TuiCommand::Scroll(-1), Some("scroll -1")),
            ),
            (
                KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE),
                (TuiCommand::EnterCommandMode(true), None),
            ),
            (
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                (TuiCommand::EnterCommandMode(false), None),
            ),
        ]);

        return Keybindings { map: keybindings };
    }

    pub fn map_keyevent_to_tuicommand(&self, event: &KeyEvent) -> Option<TuiCommand> {
        self.map.get(event).map(|(command, _)| command.clone())
    }
}
