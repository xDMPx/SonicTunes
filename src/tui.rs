use crate::SonicTunesError;
use crate::libmpv_handler::{LibMpvEventMessage, LibMpvMessage};
use ratatui::crossterm::event::{self, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    DefaultTerminal,
    style::Stylize,
    widgets::{Block, Borders},
};
use std::fmt::Write;

#[derive(Debug, Clone)]
pub enum TuiCommand {
    State(TuiState),
    Quit,
    Volume(i64),
    SetVolume(i64),
    Seek(f64),
    PlayPause,
    PlayNext,
    PlayPrevious,
    Scroll(i16),
    EnterCommandMode(bool),
    PauseAfter(u64),
    QuitAfter(u64),
}

fn map_str_to_tuicommand(str: &str) -> Option<TuiCommand> {
    if str.split_whitespace().count() > 2 {
        return None;
    }

    let mut tokens = str.split_whitespace();
    let command_str = tokens.next()?;
    let mut args = tokens;

    match command_str {
        "quit" | "q" => Some(TuiCommand::Quit),
        "vol" => {
            let arg = args.next()?;
            if arg.starts_with('-') || arg.starts_with('+') {
                let mut volume: i64 = arg.parse().ok()?;
                volume = volume.clamp(-200, 200);
                Some(TuiCommand::Volume(volume))
            } else {
                let mut volume: i64 = arg.parse().ok()?;
                volume = volume.clamp(-200, 200);
                Some(TuiCommand::SetVolume(volume))
            }
        }
        "seek" => {
            let offset: f64 = args.next()?.parse().ok()?;
            Some(TuiCommand::Seek(offset))
        }
        "play-pause" => Some(TuiCommand::PlayPause),
        "play-next" => Some(TuiCommand::PlayNext),
        "play-prev" => Some(TuiCommand::PlayPrevious),
        "pause-after" => {
            let time_min: u64 = args.next()?.parse().ok()?;
            Some(TuiCommand::PauseAfter(time_min))
        }
        "quit-after" => {
            let time_min: u64 = args.next()?.parse().ok()?;
            Some(TuiCommand::QuitAfter(time_min))
        }
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TuiState {
    Player,
    History,
    Help,
}

pub fn tui(
    libmpv_s: crossbeam::channel::Sender<LibMpvMessage>,
    tui_r: crossbeam::channel::Receiver<LibMpvEventMessage>,
) -> Result<(), SonicTunesError> {
    let mut command_mode = false;
    let mut command_text = "".to_string();
    let mut command_error = "".to_string();
    let mut cursor_position: u16 = 0;

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
            (TuiCommand::PlayNext, Some("play-prev")),
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
    let mut tui_state = TuiState::Player;

    let mut title = String::new();
    let mut artist: Option<String> = None;
    let mut terminal = ratatui::init();

    let mut history: Vec<String> = Vec::new();
    let mut current: i64 = 0;
    let mut scroll: u16 = 0;

    let mut playback_start = std::time::SystemTime::now();
    let mut playback_start_offset = 0.0;
    let mut playback_paused = true;
    let mut playback_ready = false;
    let mut playback_duration = 0;
    let mut playback_volume = 0;

    let mut pause_after = None;
    let mut quit_after = None;

    loop {
        match tui_state {
            TuiState::Player => {
                let playback_time = {
                    if !playback_ready {
                        0.0
                    } else if playback_paused {
                        playback_start_offset
                    } else {
                        playback_start_offset + playback_start.elapsed()?.as_secs_f64()
                    }
                };
                let mut playback_time = playback_time.floor() as u64;
                playback_time = playback_time.min(playback_duration);
                let symbol = {
                    if !playback_ready || playback_paused {
                        "|"
                    } else {
                        ">"
                    }
                };
                let mut to_draw = title.clone();
                if let Some(ref artist) = artist {
                    to_draw.push_str(" by ");
                    to_draw.push_str(artist);
                }
                to_draw.push_str(&format!(
                    "\n{} {} / {} vol: {}",
                    symbol,
                    secs_to_hms(playback_time),
                    secs_to_hms(playback_duration),
                    playback_volume
                ));
                draw(
                    &mut terminal,
                    &to_draw,
                    0,
                    if command_mode {
                        Some(&command_text)
                    } else {
                        None
                    },
                    if command_error.trim().is_empty() {
                        None
                    } else {
                        Some(&command_error)
                    },
                    cursor_position,
                )?;
            }
            TuiState::History => {
                let mut to_draw = "".to_string();
                let current = current as usize;
                history.iter().enumerate().for_each(|(i, x)| {
                    if i == current {
                        to_draw.push_str("* ")
                    };
                    to_draw.push_str(&format!("{x}\n"))
                });
                draw(
                    &mut terminal,
                    &to_draw,
                    scroll,
                    if command_mode {
                        Some(&command_text)
                    } else {
                        None
                    },
                    if command_error.trim().is_empty() {
                        None
                    } else {
                        Some(&command_error)
                    },
                    cursor_position,
                )?;
            }
            TuiState::Help => {
                let to_draw = generate_help_str(&keybindings);
                draw(
                    &mut terminal,
                    &to_draw,
                    scroll,
                    if command_mode {
                        Some(&command_text)
                    } else {
                        None
                    },
                    if command_error.trim().is_empty() {
                        None
                    } else {
                        Some(&command_error)
                    },
                    cursor_position,
                )?;
            }
        };

        if event::poll(std::time::Duration::from_millis(16))? {
            let event = event::read();
            if let Ok(event) = event {
                log::debug!("Event: {event:?}");
                let mut command = None;
                if let event::Event::Key(key) = event {
                    command_error = "".to_string();
                    if command_mode {
                        if key.code.to_string().len() == 1 {
                            let c = key.code.to_string().chars().next().unwrap();
                            if c.is_alphanumeric() || c == '-' || c == '+' {
                                if cursor_position == command_text.len() as u16 {
                                    command_text.push(c);
                                } else {
                                    command_text.insert(cursor_position.into(), c);
                                }
                                cursor_position += 1;
                            }
                        } else if key.code == event::KeyCode::Backspace {
                            if !command_text.is_empty() && cursor_position > 0 {
                                command_text.remove((cursor_position.saturating_sub(1)).into());
                                if cursor_position > 0 {
                                    cursor_position -= 1;
                                }
                            }
                        } else if key.code == event::KeyCode::Esc {
                            command_mode = false;
                            command_text = "".to_string();
                            cursor_position = 0;
                        } else if key.code == event::KeyCode::Enter {
                            command = map_str_to_tuicommand(&command_text);
                            if command.is_none() && !command_text.trim().is_empty() {
                                command_error = "Error: unknown command".to_string();
                            }
                            command_mode = false;
                            command_text = "".to_string();
                            cursor_position = 0;
                        } else if key.code == event::KeyCode::Char(' ') {
                            if cursor_position == command_text.len() as u16 {
                                command_text.push(' ');
                                cursor_position += 1;
                            }
                        } else if key.code == event::KeyCode::Left {
                            if cursor_position > 0 {
                                cursor_position -= 1;
                            }
                        } else if key.code == event::KeyCode::Right
                            && cursor_position < command_text.len() as u16
                        {
                            cursor_position += 1;
                        }
                    } else if let Some((key_command, _)) = keybindings.get(&key) {
                        command = Some(key_command.clone());
                    }
                    if let Some(command) = command {
                        log::debug!("Command: {command:?}");
                        match command {
                            TuiCommand::State(state) => {
                                tui_state = state.clone();
                            }
                            TuiCommand::Quit => {
                                libmpv_s.send(LibMpvMessage::Quit)?;
                                break;
                            }
                            TuiCommand::Volume(vol) => {
                                libmpv_s.send(LibMpvMessage::UpdateVolume(vol))?;
                            }
                            TuiCommand::SetVolume(vol) => {
                                libmpv_s.send(LibMpvMessage::SetVolume(vol))?;
                            }
                            TuiCommand::Seek(offset) => {
                                libmpv_s.send(LibMpvMessage::UpdatePosition(offset))?;
                            }
                            TuiCommand::PlayPause => {
                                libmpv_s.send(LibMpvMessage::PlayPause)?;
                            }
                            TuiCommand::PlayNext => {
                                libmpv_s.send(LibMpvMessage::PlayNext)?;
                            }
                            TuiCommand::PlayPrevious => {
                                libmpv_s.send(LibMpvMessage::PlayPrevious)?;
                            }
                            TuiCommand::Scroll(x) => {
                                if x > 0 && scroll < (history.len() - 1) as u16 {
                                    scroll += 1;
                                } else if x < 0 && scroll > 0 {
                                    scroll -= 1;
                                }
                            }
                            TuiCommand::EnterCommandMode(enter) => {
                                command_mode = enter;
                            }
                            TuiCommand::PauseAfter(min) => {
                                pause_after = Some(crossbeam::channel::after(
                                    std::time::Duration::from_mins(min),
                                ));
                                quit_after = None;
                            }
                            TuiCommand::QuitAfter(min) => {
                                quit_after = Some(crossbeam::channel::after(
                                    std::time::Duration::from_mins(min),
                                ));
                                pause_after = None;
                            }
                        }
                    }
                }
            }
        }
        if let Ok(rec) = tui_r.try_recv() {
            log::debug!("LibMpvEventMessage: {rec:?}");
            match rec {
                LibMpvEventMessage::StartFile => {
                    playback_ready = false;
                }
                LibMpvEventMessage::PlaybackRestart(paused) => {
                    playback_start = std::time::SystemTime::now();
                    playback_ready = true;
                    playback_paused = paused;
                }
                LibMpvEventMessage::FileLoaded(data) => {
                    playback_start = std::time::SystemTime::now();
                    playback_start_offset = 0.0;
                    playback_duration = data.duration.floor() as u64;
                    playback_volume = data.volume;
                    title = data.media_title;
                    artist = data.artist;

                    let mut entry_text = title.clone();
                    if let Some(ref artist) = artist {
                        entry_text.push_str(" by ");
                        entry_text.push_str(artist);
                    }
                    if history.is_empty() || (current as usize) >= history.len() {
                        history.push(format!("{}: {}", history.len(), entry_text));
                    }
                }
                LibMpvEventMessage::PlaybackPause => {
                    playback_start_offset += playback_start.elapsed()?.as_secs_f64();
                    playback_paused = true;
                }
                LibMpvEventMessage::PlaybackResume => {
                    playback_start = std::time::SystemTime::now();
                    playback_paused = false;
                }
                LibMpvEventMessage::VolumeUpdate(vol) => {
                    playback_volume = vol;
                }
                LibMpvEventMessage::PositionUpdate(pos) => {
                    playback_start = std::time::SystemTime::now();
                    playback_start_offset = pos;
                }
                LibMpvEventMessage::DurationUpdate(dur) => {
                    playback_duration = dur.floor() as u64;
                }
                LibMpvEventMessage::PlaylistPosUpdate(pos) => {
                    current = pos;
                }
                LibMpvEventMessage::Quit => {
                    break;
                }
            }
        }
        if pause_after
            .as_ref()
            .and_then(|x| x.try_recv().ok())
            .is_some()
        {
            libmpv_s.send(LibMpvMessage::Pause)?;
        }
        if quit_after
            .as_ref()
            .and_then(|x| x.try_recv().ok())
            .is_some()
        {
            libmpv_s.send(LibMpvMessage::Quit)?;
            break;
        }
    }
    ratatui::restore();

    Ok(())
}

pub fn draw(
    terminal: &mut DefaultTerminal,
    text: &str,
    scroll: u16,
    command: Option<&str>,
    error: Option<&str>,
    cursor_position: u16,
) -> Result<(), std::io::Error> {
    terminal.draw(|f| {
        let area = f.area();
        let block = Block::default()
            .title(env!("CARGO_PKG_NAME"))
            .borders(Borders::ALL);
        let block = block.title_alignment(ratatui::layout::Alignment::Center);
        let text = ratatui::widgets::Paragraph::new(text);
        let text = text.scroll((scroll, 0));
        let inner = block.inner(f.area());
        f.render_widget(block, area);
        f.render_widget(text, inner);
        if let Some(error) = error {
            let text = ratatui::widgets::Paragraph::new(error).light_red();
            let mut inner = inner;
            inner.y = inner.height;
            inner.height = 1;
            f.render_widget(text, inner);
        }
        if let Some(command) = command {
            let text = ratatui::widgets::Paragraph::new(":".to_owned() + command);
            let mut inner = inner;
            inner.y = inner.height;
            inner.height = 1;
            f.render_widget(text, inner);
            f.set_cursor_position(ratatui::layout::Position::new(
                inner.x + 1 + cursor_position,
                inner.y,
            ));
        }
    })?;

    Ok(())
}

fn secs_to_hms(seconds: u64) -> String {
    let h = seconds / 3600;
    let m = (seconds - h * 3600) / 60;
    let s = seconds - h * 3600 - m * 60;

    format!("{h:02}:{m:02}:{s:02}")
}

pub fn generate_help_str(
    keybindings: &std::collections::HashMap<KeyEvent, (TuiCommand, Option<&str>)>,
) -> String {
    let mut help_str = String::new();
    let min_width = 12;

    writeln!(help_str, "Commands:").unwrap();
    writeln!(help_str, "{:min_width$} {:min_width$}", "global", "quit, q").unwrap();
    writeln!(
        help_str,
        "{:min_width$} {:min_width$}",
        "global", "vol=[+|-]<i64>"
    )
    .unwrap();
    writeln!(
        help_str,
        "{:min_width$} {:min_width$}",
        "global", "seek=[+|-]<f64>"
    )
    .unwrap();
    writeln!(
        help_str,
        "{:min_width$} {:min_width$}",
        "global", "play-pause"
    )
    .unwrap();
    writeln!(
        help_str,
        "{:min_width$} {:min_width$}",
        "global", "play-next"
    )
    .unwrap();
    writeln!(
        help_str,
        "{:min_width$} {:min_width$}",
        "global", "play-prev"
    )
    .unwrap();
    writeln!(
        help_str,
        "{:min_width$} {:min_width$}",
        "global", "pause-after=<u64>"
    )
    .unwrap();
    writeln!(
        help_str,
        "{:min_width$} {:min_width$}",
        "global", "quit-after=<u64>"
    )
    .unwrap();

    help_str.push('\n');

    writeln!(help_str, "Keybindings:").unwrap();
    let mut keybindings_help_str = vec![];
    for (key_event, (_, description)) in keybindings {
        let mut help_str = String::new();
        if let Some(description) = description {
            help_str += &match key_event.code {
                KeyCode::Char(' ') => format!(
                    "{:min_width$}  {:min_width$}  {description}",
                    "global", "space"
                ),

                KeyCode::Char(c) => {
                    format!(
                        "{:min_width$}  {:min_width$}  {description}",
                        "global",
                        if key_event.modifiers == KeyModifiers::NONE {
                            c.to_string()
                        } else {
                            format!("{c}+{}", key_event.modifiers.to_string())
                        }
                    )
                }
                key_code => format!(
                    "{:min_width$}  {:min_width$}  {description}",
                    "global",
                    if key_event.modifiers == KeyModifiers::NONE {
                        key_code.to_string()
                    } else {
                        format!("{key_code}+{}", key_event.modifiers.to_string())
                    }
                ),
            };
            keybindings_help_str.push(help_str);
        }
    }

    keybindings_help_str.sort_unstable_by_key(|str| str.split("  ").last().unwrap().to_string());
    help_str.push_str(&keybindings_help_str.join("\n"));

    help_str
}
