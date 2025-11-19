use sonictunes::{audiofile_to_url, get_random_audiofile, libmpv_handler::LibMpvHandler};

fn main() {
    let url: String = std::env::args()
        .nth(1)
        .expect("Arg with SubSonicVault URL required");

    let mut mpv_handler = LibMpvHandler::initialize_libmpv(50).unwrap();
    let mpv_client = mpv_handler.create_client().unwrap();

    let audiofile = get_random_audiofile(&url);
    println!("Playing: {}", audiofile.path);
    let audiofile_url = audiofile_to_url(&url, &audiofile);
    mpv_handler.load_file(&audiofile_url).unwrap();

    let (tui_s, tui_r) = crossbeam::channel::unbounded();
    let (libmpv_s, libmpv_r) = crossbeam::channel::unbounded();

    crossbeam::scope(move |scope| {
        scope.spawn(move |_| {
            sonictunes::tui::tui(libmpv_s, tui_r);
        });
        scope.spawn(move |_| {
            mpv_handler.run(mpv_client, &url, tui_s, libmpv_r);
        });
    })
    .unwrap();
}
