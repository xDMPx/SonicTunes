#[derive(Debug, Clone)]
pub enum TuiCommand {
    State(TuiState),
    Quit,
    Volume(i64),
    SetVolume(i64),
    Seek(f64),
    SetPosition(f64),
    PlayPause,
    PlayNext,
    PlayPrevious,
    Scroll(i16),
    EnterCommandMode(bool),
    PauseAfter(u64),
    QuitAfter(u64),
    Stop,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TuiState {
    Player,
    History,
    Help,
}

fn quit(_: &mut std::str::SplitWhitespace<'_>) -> Option<TuiCommand> {
    Some(TuiCommand::Quit)
}

fn seek(args: &mut std::str::SplitWhitespace<'_>) -> Option<TuiCommand> {
    let arg = args.next()?;
    if arg.starts_with('-') || arg.starts_with('+') {
        let offset: f64 = arg.parse().ok()?;
        Some(TuiCommand::Seek(offset))
    } else {
        if let Some(pos) = arg.parse().ok() {
            Some(TuiCommand::SetPosition(pos))
        } else if arg.chars().filter(|&c| c == ':').count() == 2 {
            let (hh, mmss) = arg.split_once(':')?;
            let (mm, ss) = mmss.split_once(':')?;

            let hh: f64 = hh.parse().ok()?;
            let mm: f64 = mm.parse().ok()?;
            let ss: f64 = ss.parse().ok()?;
            let pos = (hh * 60.0 * 60.0) + (mm * 60.0) + ss;

            Some(TuiCommand::SetPosition(pos))
        } else {
            None
        }
    }
}

fn vol(args: &mut std::str::SplitWhitespace<'_>) -> Option<TuiCommand> {
    let arg = args.next()?;
    if arg.starts_with('-') || arg.starts_with('+') {
        let mut volume: i64 = arg.parse().ok()?;
        volume = volume.clamp(-200, 200);
        Some(TuiCommand::Volume(volume))
    } else {
        let mut volume: i64 = arg.parse().ok()?;
        volume = volume.clamp(-200, 200);
        Some(TuiCommand::SetVolume(volume))
    }
}

fn playpause(_: &mut std::str::SplitWhitespace<'_>) -> Option<TuiCommand> {
    Some(TuiCommand::PlayPause)
}

fn stop(_: &mut std::str::SplitWhitespace<'_>) -> Option<TuiCommand> {
    Some(TuiCommand::Stop)
}

fn playnext(_: &mut std::str::SplitWhitespace<'_>) -> Option<TuiCommand> {
    Some(TuiCommand::PlayNext)
}

fn playprev(_: &mut std::str::SplitWhitespace<'_>) -> Option<TuiCommand> {
    Some(TuiCommand::PlayPrevious)
}

fn pauseafter(args: &mut std::str::SplitWhitespace<'_>) -> Option<TuiCommand> {
    let time_min: u64 = args.next()?.parse().ok()?;
    Some(TuiCommand::PauseAfter(time_min))
}

fn quitafter(args: &mut std::str::SplitWhitespace<'_>) -> Option<TuiCommand> {
    let time_min: u64 = args.next()?.parse().ok()?;
    Some(TuiCommand::QuitAfter(time_min))
}

fn view(args: &mut std::str::SplitWhitespace<'_>) -> Option<TuiCommand> {
    let arg = args.next()?;
    match arg {
        "player" => Some(TuiCommand::State(TuiState::Player)),
        "history" => Some(TuiCommand::State(TuiState::History)),
        "help" => Some(TuiCommand::State(TuiState::Help)),
        _ => None,
    }
}

type CmdFn = fn(&mut std::str::SplitWhitespace<'_>) -> Option<TuiCommand>;

static COMMANDS: phf::Map<
    &'static str,
    fn(&mut std::str::SplitWhitespace<'_>) -> Option<TuiCommand>,
> = phf::phf_map! {
    "quit" => quit as CmdFn,
    "q" => quit as CmdFn,
    "vol" => vol as CmdFn,
    "seek" => seek as CmdFn,
    "play-pause" => playpause as CmdFn,
    "stop" => stop as CmdFn,
    "play-next" => playnext as CmdFn,
    "play-prev" => playprev as CmdFn,
    "pause-after" => pauseafter as CmdFn,
    "quit-after" => quitafter as CmdFn,
    "view" => view as CmdFn,
};

pub fn map_str_to_tuicommand(str: &str) -> Option<TuiCommand> {
    if str.split_whitespace().count() > 2 {
        return None;
    }

    let mut tokens = str.split_whitespace();
    let command_str = tokens.next()?;

    COMMANDS.get(command_str).map(|f| f(&mut tokens))?
}
