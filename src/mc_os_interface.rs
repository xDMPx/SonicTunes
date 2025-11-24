use crate::libmpv_handler::{LibMpvEventMessage, LibMpvMessage};

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
    pub fn new(libmpv_s: crossbeam::channel::Sender<LibMpvMessage>) -> Self {
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

        let mut media_controller = souvlaki::MediaControls::new(config).unwrap();

        // The closure must be Send and have a static lifetime.
        media_controller
            .attach(move |event: souvlaki::MediaControlEvent| {
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
                    _ => (),
                }
            })
            .unwrap();

        MCOSInterface {
            media_controller,
            #[cfg(target_os = "windows")]
            dummy_window,
        }
    }

    pub fn handle_signals(
        &mut self,
        tui_r: crossbeam::channel::Receiver<crate::libmpv_handler::LibMpvEventMessage>,
    ) {
        self.media_controller
            .set_playback(souvlaki::MediaPlayback::Playing { progress: None })
            .unwrap();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(16));
            if let Ok(rec) = tui_r.try_recv() {
                log::debug!("LibMpvEventMessage: {rec:?}");
                match rec {
                    LibMpvEventMessage::StartFile => {
                        self.media_controller
                            .set_playback(souvlaki::MediaPlayback::Playing { progress: None })
                            .unwrap();
                    }
                    LibMpvEventMessage::PlaybackRestart(paused) => {
                        if paused {
                            self.media_controller
                                .set_playback(souvlaki::MediaPlayback::Paused { progress: None })
                                .unwrap();
                        } else {
                            self.media_controller
                                .set_playback(souvlaki::MediaPlayback::Playing { progress: None })
                                .unwrap();
                        }
                    }
                    LibMpvEventMessage::FileLoaded(data) => {
                        self.media_controller
                            .set_playback(souvlaki::MediaPlayback::Playing { progress: None })
                            .unwrap();
                        self.media_controller
                            .set_metadata(souvlaki::MediaMetadata {
                                title: Some(&data.media_title),
                                ..Default::default()
                            })
                            .unwrap();
                    }
                    LibMpvEventMessage::PlaybackPause => {
                        self.media_controller
                            .set_playback(souvlaki::MediaPlayback::Paused { progress: None })
                            .unwrap();
                    }
                    LibMpvEventMessage::PlaybackResume => {
                        self.media_controller
                            .set_playback(souvlaki::MediaPlayback::Playing { progress: None })
                            .unwrap();
                    }
                    LibMpvEventMessage::VolumeUpdate(_) => (),
                    LibMpvEventMessage::PositionUpdate(_) => (),
                    LibMpvEventMessage::Quit => {
                        break;
                    }
                }
            }
        }
    }
}
