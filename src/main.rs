use sonictunes::{
    ProgramOption, audiofile_to_url, get_random_audiofile,
    libmpv_handler::{LibMpvEventMessage, LibMpvHandler, LibMpvMessage},
    print_help, process_args,
};

fn main() {
    let options = process_args()
        .map_err(|err| {
            match err {
                sonictunes::SonicTunesError::InvalidOption(option) => {
                    eprintln!("Provided option {option} is invalid")
                }
                sonictunes::SonicTunesError::InvalidOptionsStructure => eprintln!("Invalid input"),
                _ => panic!("{:?}", err),
            }
            print_help();
            std::process::exit(-1);
        })
        .unwrap();
    if options.contains(&ProgramOption::PrintHelp) {
        print_help();
        std::process::exit(-1);
    }

    if options.contains(&ProgramOption::Verbose) {
        simplelog::WriteLogger::init(
            simplelog::LevelFilter::Debug,
            simplelog::Config::default(),
            std::fs::File::create("debug.log").unwrap(),
        )
        .unwrap();
        log::debug!("Args: {:?}", std::env::args());
    }

    let volume = if let Some(vol) = options.iter().find_map(|o| match o {
        ProgramOption::Volume(vol) => Some(*vol),
        _ => None,
    }) {
        vol
    } else {
        50
    };

    let url = options
        .iter()
        .find_map(|o| match o {
            ProgramOption::URL(url) => Some(url),
            _ => None,
        })
        .unwrap();
    log::debug!("URL: {:?}", std::env::args());

    let mut mpv_handler = LibMpvHandler::initialize_libmpv(volume).unwrap();
    let mpv_client = mpv_handler.create_client().unwrap();

    let audiofile = get_random_audiofile(&url).unwrap();
    log::debug!("Playing: {}", audiofile.path);
    let audiofile_url = audiofile_to_url(&url, &audiofile);
    mpv_handler.load_file(&audiofile_url).unwrap();

    let (tui_s, tui_r) = crossbeam::channel::unbounded();
    let (libmpv_s, libmpv_r) = crossbeam::channel::unbounded();
    let (mc_tui_s, mc_tui_r) = crossbeam::channel::unbounded();

    let mc_tui_s2 = mc_tui_s.clone();
    let tui_s2 = tui_s.clone();
    let libmpv_s2 = libmpv_s.clone();

    let mut mc_os_interface =
        sonictunes::mc_os_interface::MCOSInterface::new(libmpv_s.clone()).unwrap();

    crossbeam::scope(move |scope| {
        scope.spawn(move |_| {
            log::debug!("TUI: START");
            sonictunes::tui::tui(libmpv_s.clone(), tui_r)
                .map_err(|err| {
                    libmpv_s.send(LibMpvMessage::Quit).unwrap();
                    mc_tui_s2.send(LibMpvEventMessage::Quit).unwrap();
                    err
                })
                .unwrap();
            log::debug!("TUI: END");
        });
        scope.spawn(move |_| {
            log::debug!("MPV: START");
            mpv_handler
                .run(mpv_client, &url, tui_s.clone(), mc_tui_s.clone(), libmpv_r)
                .map_err(|err| {
                    tui_s.send(LibMpvEventMessage::Quit).unwrap();
                    mc_tui_s.send(LibMpvEventMessage::Quit).unwrap();
                    err
                })
                .unwrap();
            log::debug!("MPV: END");
        });
        scope.spawn(move |_| {
            log::debug!("MCOSInterface: START");
            mc_os_interface
                .handle_signals(mc_tui_r)
                .map_err(|err| {
                    tui_s2.send(LibMpvEventMessage::Quit).unwrap();
                    libmpv_s2.send(LibMpvMessage::Quit).unwrap();
                    err
                })
                .unwrap();
            log::debug!("MCOSInterface: END");
        });
    })
    .unwrap();
}
