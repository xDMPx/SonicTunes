use sonictunes::{
    PingResponse, ProgramOption, audiofile_to_url, get_random_audiofile,
    libmpv_handler::{LibMpvEventMessage, LibMpvHandler, LibMpvMessage},
    print_help, process_args, reqwest_get,
};

fn main() {
    let mut log_send: Option<sonictunes::logger::LogSender> = None;
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
        let logger = sonictunes::logger::Logger::new();
        log_send = Some(sonictunes::logger::LogSender::new(logger.get_signal_send()));
        log::set_boxed_logger(Box::new(log_send.as_ref().unwrap().clone())).unwrap();
        log::set_max_level(log::LevelFilter::Trace);

        std::thread::spawn(move || {
            logger.log();
            logger.flush();
        });
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
    log::debug!("URL: {:?}", url);

    if let Ok(response) = reqwest_get(&format!("{}/ping", url.trim_end_matches('/'))) {
        let ping_response: PingResponse = response
            .json()
            .map_err(|err| eprintln!("Invalid server response, {err}"))
            .unwrap();
        if ping_response.status != "ok" {
            eprintln!("Invalid server status, {}", ping_response.status);
            std::process::exit(-1);
        }
    } else {
        eprintln!("Connection to server failed");
        std::process::exit(-1);
    }

    let mut mpv_handler = LibMpvHandler::initialize_libmpv(volume).unwrap();
    let mpv_client = mpv_handler.create_client().unwrap();

    let audiofile = get_random_audiofile(url).unwrap();
    log::debug!("Playing: {}", audiofile.path);
    let audiofile_url = audiofile_to_url(url, &audiofile);
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
                    log::error!("Tui: {:?}", err);
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
                .run(mpv_client, url, tui_s.clone(), mc_tui_s.clone(), libmpv_r)
                .map_err(|err| {
                    log::error!("MpvHandler: {:?}", err);
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
                    log::error!("MCOSInterface: {:?}", err);
                    tui_s2.send(LibMpvEventMessage::Quit).unwrap();
                    libmpv_s2.send(LibMpvMessage::Quit).unwrap();
                    err
                })
                .unwrap();
            log::debug!("MCOSInterface: END");
        });
    })
    .unwrap();
    if let Some(log_send) = log_send {
        log_send.send_quit_signal();
    }
}
