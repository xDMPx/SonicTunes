use sonictunes::{audiofile_to_url, get_random_audiofile, libmpv_handler::LibMpvHandler};

fn main() {
    let url: String = std::env::args()
        .nth(1)
        .expect("Arg with SubSonicVault URL required");

    let mpv_handler = LibMpvHandler::initialize_libmpv(50).unwrap();
    let mut mpv_client = mpv_handler.create_client().unwrap();

    let audiofile = get_random_audiofile(&url);
    println!("Playing: {}", audiofile.path);
    let audiofile_url = audiofile_to_url(&url, &audiofile);
    mpv_handler.load_file(&audiofile_url).unwrap();

    loop {
        let ev = mpv_client
            .wait_event(600.)
            .unwrap_or(Err(libmpv2::Error::Null));
        match ev {
            Ok(event) => match event {
                libmpv2::events::Event::EndFile(0) => {
                    let audiofile = get_random_audiofile(&url);
                    println!("Playing: {}", audiofile.path);
                    let audiofile_url = audiofile_to_url(&url, &audiofile);
                    mpv_handler.load_file(&audiofile_url).unwrap();
                }
                _ => println!("EV: {event:?}"),
            },
            Err(err) => {
                println!("ERR: {err:?}");
            }
        }
    }
}
