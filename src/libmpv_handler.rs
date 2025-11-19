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

        Ok(client)
    }
}
