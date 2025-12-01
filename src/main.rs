use sonictunes::{
    ProgramOption, audiofile_to_url, get_random_audiofile, libmpv_handler::LibMpvHandler,
    print_help, process_args,
};

fn main() {
    simplelog::WriteLogger::init(
        simplelog::LevelFilter::Debug,
        simplelog::Config::default(),
        std::fs::File::create("debug.log").unwrap(),
    )
    .unwrap();
    log::debug!("Args: {:?}", std::env::args());

    let options = process_args()
        .map_err(|err| {
            match err {
                sonictunes::Error::InvalidOption(option) => {
                    eprintln!("Provided option {option} is invalid")
                }
                sonictunes::Error::InvalidOptionsStructure => eprintln!("Invalid input"),
            }
            print_help();
            std::process::exit(-1);
        })
        .unwrap();
    if options.contains(&ProgramOption::PrintHelp) {
        print_help();
        std::process::exit(-1);
    }

    let url = options
        .iter()
        .find_map(|o| match o {
            ProgramOption::URL(url) => Some(url),
            _ => None,
        })
        .unwrap();
    log::debug!("URL: {:?}", std::env::args());

    let mut mpv_handler = LibMpvHandler::initialize_libmpv(50).unwrap();
    let mpv_client = mpv_handler.create_client().unwrap();

    let audiofile = get_random_audiofile(&url);
    log::debug!("Playing: {}", audiofile.path);
    let audiofile_url = audiofile_to_url(&url, &audiofile);
    mpv_handler.load_file(&audiofile_url).unwrap();

    let (tui_s, tui_r) = crossbeam::channel::unbounded();
    let (libmpv_s, libmpv_r) = crossbeam::channel::unbounded();
    let (mc_tui_s, mc_tui_r) = crossbeam::channel::unbounded();

    let mut mc_os_interface = sonictunes::mc_os_interface::MCOSInterface::new(libmpv_s.clone());

    crossbeam::scope(move |scope| {
        scope.spawn(move |_| {
            log::debug!("TUI: START");
            sonictunes::tui::tui(libmpv_s, tui_r);
            log::debug!("TUI: END");
        });
        scope.spawn(move |_| {
            log::debug!("MPV: START");
            mpv_handler.run(mpv_client, &url, tui_s, mc_tui_s, libmpv_r);
            log::debug!("MPV: END");
        });
        scope.spawn(move |_| {
            log::debug!("MCOSInterface: START");
            mc_os_interface.handle_signals(mc_tui_r);
            log::debug!("MCOSInterface: END");
        });
    })
    .unwrap();
}
