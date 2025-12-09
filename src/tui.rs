use crate::SonicTunesError;
use crate::libmpv_handler::{LibMpvEventMessage, LibMpvMessage};
use ratatui::crossterm::event::{self, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    DefaultTerminal,
    widgets::{Block, Borders},
};

#[derive(Debug)]
pub enum TuiCommand {
    Quit,
    Volume(i64),
    Seek(f64),
    PlayPause,
    PlayNext,
    PlayPrevious,
}

pub fn tui(
    libmpv_s: crossbeam::channel::Sender<LibMpvMessage>,
    tui_r: crossbeam::channel::Receiver<LibMpvEventMessage>,
) -> Result<(), SonicTunesError> {
    let commands = std::collections::HashMap::from([
        (
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
            TuiCommand::Quit,
        ),
        (
            KeyEvent::new(KeyCode::Char('{'), KeyModifiers::NONE),
            TuiCommand::Volume(-1),
        ),
        (
            KeyEvent::new(KeyCode::Char('}'), KeyModifiers::NONE),
            TuiCommand::Volume(1),
        ),
        (
            KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE),
            TuiCommand::Volume(-10),
        ),
        (
            KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE),
            TuiCommand::Volume(10),
        ),
        (
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            TuiCommand::Seek(-10.0),
        ),
        (
            KeyEvent::new(KeyCode::Left, KeyModifiers::SHIFT),
            TuiCommand::Seek(-60.0),
        ),
        (
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            TuiCommand::Seek(10.0),
        ),
        (
            KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT),
            TuiCommand::Seek(60.0),
        ),
        (
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
            TuiCommand::PlayPause,
        ),
        (
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE),
            TuiCommand::PlayPrevious,
        ),
        (
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
            TuiCommand::PlayNext,
        ),
    ]);

    let mut title = String::new();
    let mut artist: Option<String> = None;
    let mut terminal = ratatui::init();

    let mut playback_start = std::time::SystemTime::now();
    let mut playback_start_offset = 0.0;
    let mut playback_paused = true;
    let mut playback_ready = false;
    let mut playback_duration = 0;
    let mut playback_volume = 0;

    loop {
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
        draw(&mut terminal, &to_draw)?;

        if event::poll(std::time::Duration::from_millis(16))? {
            let event = event::read();
            if let Ok(event) = event {
                log::debug!("Event: {event:?}");
                if let event::Event::Key(key) = event {
                    if let Some(command) = commands.get(&key) {
                        match command {
                            TuiCommand::Quit => {
                                libmpv_s.send(LibMpvMessage::Quit)?;
                                break;
                            }
                            TuiCommand::Volume(vol) => {
                                libmpv_s.send(LibMpvMessage::UpdateVolume(*vol))?;
                            }
                            TuiCommand::Seek(offset) => {
                                libmpv_s.send(LibMpvMessage::UpdatePosition(*offset))?;
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
                LibMpvEventMessage::Quit => (),
            }
        }
    }
    ratatui::restore();

    Ok(())
}

pub fn draw(terminal: &mut DefaultTerminal, text: &str) -> Result<(), std::io::Error> {
    terminal.draw(|f| {
        let area = f.area();
        let block = Block::default()
            .title(env!("CARGO_PKG_NAME"))
            .borders(Borders::ALL);
        let block = block.title_alignment(ratatui::layout::Alignment::Center);
        let text = ratatui::widgets::Paragraph::new(text);
        let inner = block.inner(f.area());
        f.render_widget(block, area);
        f.render_widget(text, inner);
    })?;

    Ok(())
}

fn secs_to_hms(seconds: u64) -> String {
    let h = seconds / 3600;
    let m = (seconds - h * 3600) / 60;
    let s = seconds - h * 3600 - m * 60;

    format!("{h:02}:{m:02}:{s:02}")
}
