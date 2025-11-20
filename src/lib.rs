use rand::random_range;

pub mod libmpv_handler;
pub mod mc_os_interface;
pub mod tui;

#[derive(serde::Deserialize, Clone)]
pub struct AudioFile {
    pub id: u64,
    pub path: String,
    pub mime: String,
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

pub fn get_random_audiofile(url: &str) -> AudioFile {
    let mut url_files = url.trim_end_matches('/').to_string();
    url_files.push_str("/files");
    let files_response = reqwest_get(&url_files).unwrap();
    let audiofiles = files_response.json::<Vec<AudioFile>>().unwrap();
    let id = random_range(0..audiofiles.len());

    audiofiles[id].clone()
}

pub fn audiofile_to_url(url: &str, audiofile: &AudioFile) -> String {
    let mut audiofile_url = url.trim_end_matches('/').to_string();
    audiofile_url = format!("{audiofile_url}/file/{}", audiofile.id);

    audiofile_url
}
