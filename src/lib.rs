use rand::random_range;

use crate::libmpv_handler::LibMpvMessage;

pub mod libmpv_handler;
pub mod mc_os_interface;
pub mod tui;

#[derive(serde::Deserialize, Clone)]
pub struct AudioFile {
    pub id: String,
    pub path: String,
    pub mime: String,
}

#[derive(PartialEq)]
pub enum ProgramOption {
    URL(String),
    PrintHelp,
    Volume(i64),
    Verbose,
}

#[derive(Debug)]
pub enum SonicTunesError {
    InvalidOption(String),
    InvalidOptionsStructure,
    ReqwestError(reqwest::Error),
    SouvlakiError(souvlaki::Error),
    SystemTimeError(std::time::SystemTimeError),
    IOError(std::io::Error),
    LibMpvMessageSendError(crossbeam::channel::SendError<LibMpvMessage>),
}

impl From<reqwest::Error> for SonicTunesError {
    fn from(err: reqwest::Error) -> Self {
        SonicTunesError::ReqwestError(err)
    }
}

impl From<souvlaki::Error> for SonicTunesError {
    fn from(err: souvlaki::Error) -> Self {
        SonicTunesError::SouvlakiError(err)
    }
}

impl From<std::time::SystemTimeError> for SonicTunesError {
    fn from(err: std::time::SystemTimeError) -> Self {
        SonicTunesError::SystemTimeError(err)
    }
}

impl From<std::io::Error> for SonicTunesError {
    fn from(err: std::io::Error) -> Self {
        SonicTunesError::IOError(err)
    }
}

impl From<crossbeam::channel::SendError<LibMpvMessage>> for SonicTunesError {
    fn from(err: crossbeam::channel::SendError<LibMpvMessage>) -> Self {
        SonicTunesError::LibMpvMessageSendError(err)
    }
}

pub fn process_args() -> Result<Vec<ProgramOption>, SonicTunesError> {
    let mut options = vec![];
    let mut args: Vec<String> = std::env::args().skip(1).collect();

    let last_arg = args.pop().ok_or(SonicTunesError::InvalidOptionsStructure)?;
    if last_arg != "--help" {
        let url = last_arg;
        if !url.starts_with("http") {
            return Err(SonicTunesError::InvalidOptionsStructure);
        }
        options.push(ProgramOption::URL(url));
    } else {
        args.push(last_arg);
    }

    for arg in args {
        let arg = match arg.as_str() {
            "--help" => Ok(ProgramOption::PrintHelp),
            "--verbose" => Ok(ProgramOption::Verbose),
            s if s.starts_with("--volume=") => {
                if let Some(Ok(vol)) = s.split_once('=').map(|(_, s)| s.parse::<i8>()) {
                    if vol >= 0 && vol <= 100 {
                        Ok(ProgramOption::Volume(vol.into()))
                    } else {
                        Err(SonicTunesError::InvalidOption(arg))
                    }
                } else {
                    Err(SonicTunesError::InvalidOption(arg))
                }
            }
            _ => Err(SonicTunesError::InvalidOption(arg)),
        };
        options.push(arg?);
    }

    Ok(options)
}

pub fn print_help() {
    println!(
        "Usage: {} [OPTIONS] SUBSONICVAULT_URL",
        env!("CARGO_PKG_NAME")
    );
    println!("       {} --help", env!("CARGO_PKG_NAME"));
    println!("Options:");
    println!("\t --volume=<value>\t(0..100)");
    println!("\t --verbose");
    println!("\t --help");
}

#[inline(always)]
pub fn get_reqwest_client() -> reqwest::Result<reqwest::blocking::Client> {
    let user_agent: String = format!("{}/{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    let reqwest_client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(1))
        .user_agent(user_agent)
        .build()?;

    Ok(reqwest_client)
}

#[inline(always)]
pub fn reqwest_get(url: &str) -> reqwest::Result<reqwest::blocking::Response> {
    let reqwest_client = get_reqwest_client()?;

    let request = reqwest_client.get(url).build()?;
    let response = reqwest_client.execute(request)?;

    Ok(response)
}

pub fn get_random_audiofile(url: &str) -> Result<AudioFile, SonicTunesError> {
    let mut url_files = url.trim_end_matches('/').to_string();
    url_files.push_str("/files");
    let files_response = reqwest_get(&url_files)?;
    let audiofiles = files_response.json::<Vec<AudioFile>>()?;
    let id = random_range(0..audiofiles.len());

    Ok(audiofiles[id].clone())
}

pub fn audiofile_to_url(url: &str, audiofile: &AudioFile) -> String {
    let mut audiofile_url = url.trim_end_matches('/').to_string();
    audiofile_url = format!("{audiofile_url}/file/{}", audiofile.id);

    audiofile_url
}
