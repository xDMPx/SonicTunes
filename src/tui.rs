mod commands;
mod keybindings;

use crate::SonicTunesError;
use crate::libmpv_handler::{LibMpvEventMessage, LibMpvMessage};
use crate::tui::commands::{
    TuiCommand, TuiState, generate_completion_suggestions, map_str_to_tuicommand,
};
use ratatui::crossterm::event::{self, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    DefaultTerminal,
    style::Stylize,
    widgets::{Block, Borders},
};
use std::fmt::Write;

pub fn tui(
    libmpv_s: crossbeam::channel::Sender<LibMpvMessage>,
    tui_r: crossbeam::channel::Receiver<LibMpvEventMessage>,
) -> Result<(), SonicTunesError> {
    let mut command_mode = false;
    let mut command_text = "".to_string();
    let mut command_error = "".to_string();
    let mut cursor_position: u16 = 0;
    let mut command_suggestions: Option<Vec<&str>> = None;
    let mut command_suggestions_index: Option<usize> = None;

    let keybindings = keybindings::Keybindings::new();
    let mut tui_state = TuiState::Player;

    let mut title = String::new();
    let mut artist: Option<String> = None;
    let mut terminal = ratatui::init();

    let mut history: Vec<String> = Vec::new();
    let mut current: i64 = 0;
    let mut scroll: u16 = 0;
    let mut scroll_to_center: bool = false;

    let mut playback_start = std::time::SystemTime::now();
    let mut playback_start_offset = 0.0;
    let mut playback_paused = true;
    let mut playback_ready = false;
    let mut playback_duration = 0;
    let mut playback_volume = 0;

    let mut pause_after = None;
    let mut pause_after_timer: Option<std::time::SystemTime> = None;
    let mut pause_after_duration: Option<std::time::Duration> = None;
    let mut quit_after = None;
    let mut quit_after_timer: Option<std::time::SystemTime> = None;
    let mut quit_after_duration: Option<std::time::Duration> = None;

    loop {
        let mut timer_text = None;
        if let Some(pause_after_timer) = pause_after_timer {
            let elapsed = pause_after_timer.elapsed();
            let pause_after_duration = pause_after_duration.unwrap();
            if let Ok(elapsed) = elapsed {
                let pause_time_left: std::time::Duration =
                    pause_after_duration.saturating_sub(elapsed);
                timer_text = Some(format!("P: {}", secs_to_hms(pause_time_left.as_secs())));
            }
        }
        if let Some(quit_after_timer) = quit_after_timer {
            let elapsed = quit_after_timer.elapsed();
            let quit_after_duration = quit_after_duration.unwrap();
            if let Ok(elapsed) = elapsed {
                let quit_time_left: std::time::Duration =
                    quit_after_duration.saturating_sub(elapsed);
                timer_text = Some(format!("Q: {}", secs_to_hms(quit_time_left.as_secs())));
            }
        }

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
                    &mut 0,
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
                    timer_text.as_deref(),
                    false,
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
                    &mut scroll,
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
                    timer_text.as_deref(),
                    scroll_to_center,
                )?;
                scroll_to_center = false;
            }
            TuiState::Help => {
                let min_width = 12;
                let mut to_draw = generate_help_str(min_width);
                writeln!(to_draw, "Keybindings:").unwrap();
                writeln!(to_draw, "{}", keybindings.generate_help_str(min_width)).unwrap();
                draw(
                    &mut terminal,
                    &to_draw,
                    &mut scroll,
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
                    timer_text.as_deref(),
                    false,
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
                        if key.code != event::KeyCode::Tab && key.code != event::KeyCode::BackTab {
                            command_suggestions_index = None;
                            command_suggestions = None;
                        }

                        if key.code.to_string().len() == 1 {
                            let c = key.code.to_string().chars().next().unwrap();
                            if c.is_alphanumeric() || c == '-' || c == '+' || c == ':' {
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
                        } else if key.code == event::KeyCode::Tab
                            || key.code == event::KeyCode::BackTab
                        {
                            if command_suggestions.is_none() {
                                let suggestions = generate_completion_suggestions(&command_text);
                                if !suggestions.is_empty() {
                                    command_suggestions = Some(suggestions);
                                }
                            }
                            if let Some(ref suggestions) = command_suggestions {
                                let i = if key.code == event::KeyCode::Tab {
                                    command_suggestions_index.map_or(0, |i| {
                                        if i < suggestions.len() - 1 { i + 1 } else { 0 }
                                    })
                                } else {
                                    command_suggestions_index.map_or(suggestions.len() - 1, |i| {
                                        if i != 0 { i - 1 } else { suggestions.len() - 1 }
                                    })
                                };

                                command_suggestions_index = Some(i);
                                let suggestion = suggestions.get(i).unwrap().to_owned();
                                command_text = suggestion.to_string();
                                cursor_position = command_text.len() as u16;
                            }
                        }
                    } else if let Some(key_command) = keybindings.map_keyevent_to_tuicommand(&key) {
                        command = Some(key_command.clone());
                    }
                    if let Some(command) = command {
                        log::debug!("Command: {command:?}");
                        match command {
                            TuiCommand::State(state) => {
                                tui_state = state.clone();
                                scroll_to_center = true;
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
                            TuiCommand::SetPosition(pos) => {
                                libmpv_s.send(LibMpvMessage::SetPosition(pos))?;
                            }
                            TuiCommand::PlayPause => {
                                libmpv_s.send(LibMpvMessage::PlayPause)?;
                            }
                            TuiCommand::Stop => {
                                libmpv_s.send(LibMpvMessage::Stop)?;
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
                                pause_after_duration = Some(std::time::Duration::from_mins(min));
                                pause_after_timer = Some(std::time::SystemTime::now());
                                quit_after = None;
                                quit_after_duration = None;
                                quit_after_timer = None;
                            }
                            TuiCommand::QuitAfter(min) => {
                                quit_after = Some(crossbeam::channel::after(
                                    std::time::Duration::from_mins(min),
                                ));
                                quit_after_duration = Some(std::time::Duration::from_mins(min));
                                quit_after_timer = Some(std::time::SystemTime::now());
                                pause_after = None;
                                pause_after_duration = None;
                                pause_after_timer = None;
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
    scroll: &mut u16,
    command: Option<&str>,
    error: Option<&str>,
    cursor_position: u16,
    timer_text: Option<&str>,
    scroll_to_center: bool,
) -> Result<(), std::io::Error> {
    terminal.draw(|f| {
        let area = f.area();
        let block = Block::default()
            .title(env!("CARGO_PKG_NAME"))
            .borders(Borders::ALL);
        let block = block.title_alignment(ratatui::layout::Alignment::Center);
        if scroll_to_center {
            if let Some((i, _)) = text.lines().enumerate().find(|(_, l)| l.starts_with('*')) {
                let i = i as u16;
                let inner = block.inner(f.area());
                if i > (inner.height / 2) {
                    *scroll = i - (inner.height / 2);
                } else {
                    *scroll = 0;
                }
            }
        }
        let text = ratatui::widgets::Paragraph::new(text);
        let text = text.scroll((*scroll, 0));
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
        if let Some(timer_text) = timer_text {
            let text = ratatui::widgets::Paragraph::new(timer_text);
            let mut inner = inner;
            inner.y = inner.height;
            inner.x = inner.width - timer_text.chars().count() as u16;
            inner.height = 1;
            f.render_widget(text, inner);
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

pub fn generate_help_str(min_width: usize) -> String {
    let mut help_str = String::new();

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
    writeln!(help_str, "{:min_width$} {:min_width$}", "global", "stop").unwrap();
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
    writeln!(
        help_str,
        "{:min_width$} {:min_width$}",
        "global", "view <player|history|help>"
    )
    .unwrap();

    help_str.push('\n');

    help_str
}
