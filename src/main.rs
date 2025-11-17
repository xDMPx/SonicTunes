fn main() {
    let url: String = std::env::args()
        .nth(1)
        .expect("Arg with SubSonicVault URL required");

    let mpv_handler = LibMpvHandler::initialize_libmpv(50).unwrap();
    mpv_handler.load_file(&url).unwrap();

    loop {
        if let Ok(mut mpv_client) = mpv_handler.mpv.create_client(None) {
            let ev = mpv_client
                .wait_event(600.)
                .unwrap_or(Err(libmpv2::Error::Null));
            match ev {
                Ok(event) => match event {
                    libmpv2::events::Event::EndFile(0) => {
                        println!("Playing next file:");
                        mpv_handler.load_file(&url).unwrap();
                    }
                    _ => println!("EV: {event:?}"),
                },
                Err(err) => {
                    println!("ERR: {err:?}");
                }
            }
        }
    }
}

struct LibMpvHandler {
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
}
