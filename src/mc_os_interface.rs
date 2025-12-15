use crate::{
    SonicTunesError,
    libmpv_handler::{LibMpvEventMessage, LibMpvMessage},
};

#[derive(Debug)]
pub enum MCOSInterfaceSignals {
    Pause,
    Resume,
    PlayNext,
    PlayPrev,
    UpdateMetadataTitle(String),
    End,
}

pub struct MCOSInterface {
    media_controller: souvlaki::MediaControls,
    #[cfg(target_os = "windows")]
    #[allow(dead_code)]
    dummy_window: windows_async::DummyWindow,
}

impl MCOSInterface {
    pub fn new(
        libmpv_s: crossbeam::channel::Sender<LibMpvMessage>,
    ) -> Result<Self, SonicTunesError> {
        #[cfg(not(target_os = "windows"))]
        let hwnd = None;

        #[cfg(target_os = "windows")]
        let dummy_window = windows_async::create_dummy_window();
        #[cfg(target_os = "windows")]
        let hwnd = {
            use std::os::raw::c_void;
            let hwnd = dummy_window.hwnd().0.to_owned() as *mut c_void;
            Some(hwnd)
        };

        let config = souvlaki::PlatformConfig {
            dbus_name: "sonic_tunes",
            display_name: "SonicTunes",
            hwnd,
        };

        let mut media_controller = souvlaki::MediaControls::new(config)?;

        // The closure must be Send and have a static lifetime.
        media_controller.attach(move |event: souvlaki::MediaControlEvent| {
            log::debug!("MediaControlEvent: {event:?}");
            match event {
                souvlaki::MediaControlEvent::Play => {
                    libmpv_s.send(LibMpvMessage::Resume).unwrap();
                }
                souvlaki::MediaControlEvent::Pause => {
                    libmpv_s.send(LibMpvMessage::Pause).unwrap();
                }
                souvlaki::MediaControlEvent::Previous => {
                    libmpv_s.send(LibMpvMessage::PlayPrevious).unwrap();
                }
                souvlaki::MediaControlEvent::Next => {
                    libmpv_s.send(LibMpvMessage::PlayNext).unwrap();
                }
                souvlaki::MediaControlEvent::Toggle => {
                    libmpv_s.send(LibMpvMessage::PlayPause).unwrap();
                }
                souvlaki::MediaControlEvent::SetVolume(vol) => {
                    libmpv_s
                        .send(LibMpvMessage::SetVolume((vol * 100.0).floor() as i64))
                        .unwrap();
                }
                souvlaki::MediaControlEvent::SeekBy(direction, duration) => {
                    let offset = match direction {
                        souvlaki::SeekDirection::Forward => duration.as_secs_f64(),
                        souvlaki::SeekDirection::Backward => -duration.as_secs_f64(),
                    };
                    libmpv_s
                        .send(LibMpvMessage::UpdatePosition(offset))
                        .unwrap();
                }
                souvlaki::MediaControlEvent::SetPosition(pos) => {
                    libmpv_s
                        .send(LibMpvMessage::SetPosition(pos.0.as_secs_f64()))
                        .unwrap();
                }
                _ => (),
            }
        })?;

        Ok(MCOSInterface {
            media_controller,
            #[cfg(target_os = "windows")]
            dummy_window,
        })
    }

    pub fn handle_signals(
        &mut self,
        tui_r: crossbeam::channel::Receiver<crate::libmpv_handler::LibMpvEventMessage>,
    ) -> Result<(), SonicTunesError> {
        let mut title = String::new();
        let mut artist: Option<String> = None;
        let mut album: Option<String> = None;
        let mut playback_start = std::time::SystemTime::now();
        let mut playback_start_offset = 0.0;
        let mut playback_paused = true;
        let mut playback_ready = false;

        let mut update_playback_timer = std::time::SystemTime::now();

        self.media_controller
            .set_playback(souvlaki::MediaPlayback::Playing { progress: None })?;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(16));
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
                        self.media_controller
                            .set_metadata(souvlaki::MediaMetadata {
                                title: Some(&data.media_title),
                                artist: data.artist.as_deref(),
                                album: data.album.as_deref(),
                                ..Default::default()
                            })?;
                        title = data.media_title;
                        artist = data.artist;
                        album = data.album;
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
                        #[cfg(target_os = "linux")]
                        self.media_controller.set_volume((vol as f64) / 100.0)?;
                    }
                    LibMpvEventMessage::PositionUpdate(pos) => {
                        playback_start = std::time::SystemTime::now();
                        playback_start_offset = pos;
                    }
                    LibMpvEventMessage::DurationUpdate(dur) => {
                        self.media_controller
                            .set_metadata(souvlaki::MediaMetadata {
                                title: Some(&title),
                                artist: artist.as_deref(),
                                album: album.as_deref(),
                                duration: Some(std::time::Duration::from_secs_f64(dur)),
                                ..Default::default()
                            })?;
                    }
                    LibMpvEventMessage::Quit => {
                        break;
                    }
                    LibMpvEventMessage::PlaylistPosUpdate(_) => (),
                }
            }

            if update_playback_timer.elapsed()?.as_secs_f64() > 0.5 {
                update_playback_timer = std::time::SystemTime::now();
                let playback_time = {
                    if !playback_ready {
                        0.0
                    } else if playback_paused {
                        playback_start_offset
                    } else {
                        playback_start_offset + playback_start.elapsed()?.as_secs_f64()
                    }
                };
                if playback_paused {
                    self.media_controller
                        .set_playback(souvlaki::MediaPlayback::Paused {
                            progress: Some(souvlaki::MediaPosition(
                                std::time::Duration::from_secs_f64(playback_time),
                            )),
                        })?;
                } else {
                    self.media_controller
                        .set_playback(souvlaki::MediaPlayback::Playing {
                            progress: Some(souvlaki::MediaPosition(
                                std::time::Duration::from_secs_f64(playback_time),
                            )),
                        })?;
                }
            }
        }

        Ok(())
    }
}
