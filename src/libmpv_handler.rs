use crate::{audiofile_to_url, get_random_audiofile};

#[derive(Debug)]
pub enum LibMpvMessage {
    Quit,
    UpdateVolume(i64),
    SetVolume(i64),
    UpdatePosition(f64),
    SetPosition(f64),
    Resume,
    Pause,
    PlayPause,
    PlayNext,
    PlayPrevious,
}

#[derive(Debug)]
pub enum LibMpvEventMessage {
    StartFile,
    PlaybackRestart(bool),
    PlaybackPause,
    PlaybackResume,
    FileLoaded(FileLoadedData),
    VolumeUpdate(i64),
    PositionUpdate(f64),
    DurationUpdate(f64),
    Quit,
}

#[derive(Debug)]
pub struct FileLoadedData {
    pub media_title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: f64,
    pub volume: i64,
}

pub struct LibMpvHandler {
    mpv: libmpv2::Mpv,
}

impl LibMpvHandler {
    pub fn initialize_libmpv(volume: i64) -> Result<Self, libmpv2::Error> {
        let mpv = libmpv2::Mpv::new()?;
        mpv.set_property("volume", volume)?;
        mpv.set_property("vo", "null")?;

        mpv.disable_deprecated_events()?;

        Ok(LibMpvHandler { mpv })
    }

    pub fn load_file(&self, file: &str) -> Result<(), libmpv2::Error> {
        self.mpv.command("loadfile", &[file, "append-play"])
    }

    pub fn create_client(&self) -> Result<libmpv2::Mpv, libmpv2::Error> {
        let client = self.mpv.create_client(None)?;
        client.disable_deprecated_events()?;

        client.observe_property("pause", libmpv2::Format::Flag, 0)?;
        client.observe_property("volume", libmpv2::Format::Int64, 0)?;
        client.observe_property("duration/full", libmpv2::Format::Double, 0)?;

        Ok(client)
    }

    pub fn run(
        &mut self,
        mut mpv_client: libmpv2::Mpv,
        url: &str,
        tui_s: crossbeam::channel::Sender<LibMpvEventMessage>,
        mc_os_s: crossbeam::channel::Sender<LibMpvEventMessage>,
        libmpv_r: crossbeam::channel::Receiver<LibMpvMessage>,
    ) {
        loop {
            let ev = mpv_client
                .wait_event(0.016)
                .unwrap_or(Err(libmpv2::Error::Null));

            if ev.is_ok() {
                log::debug!("Event {ev:?}");
            }
            match ev {
                Ok(event) => match event {
                    libmpv2::events::Event::StartFile => {
                        tui_s.send(LibMpvEventMessage::StartFile).unwrap();
                        mc_os_s.send(LibMpvEventMessage::StartFile).unwrap();
                    }
                    libmpv2::events::Event::PlaybackRestart => {
                        let pause = self.mpv.get_property::<bool>("pause").unwrap();
                        tui_s
                            .send(LibMpvEventMessage::PlaybackRestart(pause))
                            .unwrap();
                        mc_os_s
                            .send(LibMpvEventMessage::PlaybackRestart(pause))
                            .unwrap();
                    }
                    libmpv2::events::Event::PropertyChange {
                        name: "pause",
                        change: libmpv2::events::PropertyData::Flag(pause),
                        ..
                    } => {
                        if pause {
                            tui_s.send(LibMpvEventMessage::PlaybackPause).unwrap();
                            mc_os_s.send(LibMpvEventMessage::PlaybackPause).unwrap();
                        } else {
                            tui_s.send(LibMpvEventMessage::PlaybackResume).unwrap();
                            mc_os_s.send(LibMpvEventMessage::PlaybackResume).unwrap();
                        }
                    }
                    libmpv2::events::Event::PropertyChange {
                        name: "volume",
                        change: libmpv2::events::PropertyData::Int64(volume),
                        ..
                    } => {
                        tui_s
                            .send(LibMpvEventMessage::VolumeUpdate(volume))
                            .unwrap();
                        mc_os_s
                            .send(LibMpvEventMessage::VolumeUpdate(volume))
                            .unwrap();
                    }
                    libmpv2::events::Event::PropertyChange {
                        name: "duration/full",
                        change: libmpv2::events::PropertyData::Double(duration),
                        ..
                    } => {
                        tui_s
                            .send(LibMpvEventMessage::DurationUpdate(duration))
                            .unwrap();
                        mc_os_s
                            .send(LibMpvEventMessage::DurationUpdate(duration))
                            .unwrap();
                    }
                    libmpv2::events::Event::Seek => {
                        let time_pos = self.mpv.get_property::<f64>("time-pos/full").unwrap();
                        tui_s
                            .send(LibMpvEventMessage::PositionUpdate(time_pos))
                            .unwrap();
                        mc_os_s
                            .send(LibMpvEventMessage::PositionUpdate(time_pos))
                            .unwrap();
                    }
                    libmpv2::events::Event::FileLoaded => {
                        let media_title = self
                            .mpv
                            .get_property::<libmpv2::MpvStr>("metadata/by-key/title")
                            .unwrap_or_else(|_| {
                                self.mpv
                                    .get_property::<libmpv2::MpvStr>("media-title")
                                    .unwrap()
                            })
                            .to_string();
                        let artist = self
                            .mpv
                            .get_property::<libmpv2::MpvStr>("metadata/by-key/artist")
                            .map(|s| Some(s.to_string()))
                            .unwrap_or_else(|_| None);
                        let album = self
                            .mpv
                            .get_property::<libmpv2::MpvStr>("metadata/by-key/album")
                            .map(|s| Some(s.to_string()))
                            .unwrap_or_else(|_| None);

                        let duration = self.mpv.get_property::<f64>("duration/full").unwrap_or(0.0);
                        let volume = self.mpv.get_property::<i64>("volume").unwrap();
                        tui_s
                            .send(LibMpvEventMessage::FileLoaded(FileLoadedData {
                                media_title: media_title.clone(),
                                artist: artist.clone(),
                                album: album.clone(),
                                duration,
                                volume,
                            }))
                            .unwrap();
                        mc_os_s
                            .send(LibMpvEventMessage::FileLoaded(FileLoadedData {
                                media_title,
                                artist,
                                album,
                                duration,
                                volume,
                            }))
                            .unwrap();
                    }
                    libmpv2::events::Event::EndFile(0) => {
                        let audiofile = get_random_audiofile(&url);
                        let audiofile_url = audiofile_to_url(&url, &audiofile);
                        self.load_file(&audiofile_url).unwrap();
                    }

                    _ => (),
                },
                Err(_err) => {
                    //println!("ERR: {err:?}");
                }
            }

            if let Ok(msg) = libmpv_r.try_recv() {
                log::debug!("LibMpvMessage: {msg:?}");
                match msg {
                    LibMpvMessage::Quit => {
                        mc_os_s.send(LibMpvEventMessage::Quit).unwrap();
                        self.mpv.command("quit", &["0"]).unwrap();
                        break;
                    }
                    LibMpvMessage::UpdateVolume(vol) => {
                        let mut volume = self.mpv.get_property::<i64>("volume").unwrap();
                        volume += vol;
                        volume = volume.clamp(0, 200);
                        self.mpv.set_property("volume", volume).unwrap();
                    }
                    LibMpvMessage::SetPosition(pos) => {
                        self.mpv
                            .command("seek", &[&pos.to_string(), "absolute"])
                            .unwrap();
                    }
                    LibMpvMessage::SetVolume(vol) => {
                        self.mpv.set_property("volume", vol).unwrap();
                    }
                    LibMpvMessage::UpdatePosition(offset) => {
                        self.mpv.command("seek", &[&offset.to_string()]).unwrap();
                    }
                    LibMpvMessage::PlayPause => {
                        self.mpv.command("cycle", &["pause"]).unwrap();
                    }
                    LibMpvMessage::Resume => {
                        self.mpv.set_property("pause", false).unwrap();
                    }
                    LibMpvMessage::Pause => {
                        self.mpv.set_property("pause", true).unwrap();
                    }
                    LibMpvMessage::PlayNext => {
                        if let Err(err) = self.mpv.command("playlist-next", &["weak"]) {
                            if err != libmpv2::Error::Raw(-12) {
                                panic!("{err:?}");
                            } else {
                                let pos = self
                                    .mpv
                                    .get_property::<i64>("playlist-playing-pos")
                                    .unwrap();
                                let count = self.mpv.get_property::<i64>("playlist-count").unwrap();
                                if pos == count - 1 {
                                    let audiofile = get_random_audiofile(&url);
                                    let audiofile_url = audiofile_to_url(&url, &audiofile);
                                    self.load_file(&audiofile_url).unwrap();
                                }
                                self.mpv.command("playlist-next", &["force"]).unwrap();
                            }
                        }
                    }
                    LibMpvMessage::PlayPrevious => {
                        if let Err(err) = self.mpv.command("playlist-prev", &["weak"]) {
                            if err != libmpv2::Error::Raw(-12) {
                                panic!("{err:?}");
                            } else {
                                self.mpv.command("seek", &["0", "absolute"]).unwrap();
                            }
                        }
                    }
                }
            }
        }
    }
}
