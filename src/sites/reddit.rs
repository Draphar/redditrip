/*
 * Copyright 2020 Joshua Prieth
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

/*!
Support for reddit downloads.

# Domains

- `i.redd.it`
- `v.redd.it`
*/

use std::path::Path;

use http::Uri;
use std::process::Stdio;
use tokio::{fs, process::Command};

use crate::prelude::*;
use crate::sites::pushshift::SecureMedia;
use std::io::ErrorKind;

/// Specifies how videos from `v.redd.it` are downloaded.
#[derive(Debug)]
pub enum VRedditMode {
    /// Leave out the audio.
    NoAudio,

    /// Use ffmpeg to combine the audio and video.
    Ffmpeg,

    /// Use a website to download the video.
    /// The characters `{}` are replaced by the ID.
    Website(String),
}

impl<'a> From<&'a str> for VRedditMode {
    fn from(s: &str) -> Self {
        match s {
            "no-audio" => VRedditMode::NoAudio,
            "ffmpeg" => VRedditMode::Ffmpeg,
            other => VRedditMode::Website(other.to_string()),
        }
    }
}

/// Fetches an image from `i.redd.it`.
pub async fn fetch_image(client: &Client, url: &Uri, output: &Path) -> Result<()> {
    trace!("fetch({:?}, {:?})", url, output);

    download(client, url, output).await
}

/// Fetches a video from `v.redd.it`.
pub async fn fetch_video(
    client: &Client,
    url: &Uri,
    output: &Path,
    temp_dir: &Path,
    vreddit_mode: &VRedditMode,
    media: &Option<SecureMedia>,
) -> Result<()> {
    let media = &media
        .as_ref()
        .and_then(|media| media.reddit_video.as_ref())
        .ok_or_else(|| Error::new("No downloadable media found"))?;

    let id = &url.path()[1..];

    match vreddit_mode {
        VRedditMode::NoAudio => no_audio(client, &media.fallback_url, output).await,
        VRedditMode::Ffmpeg => ffmpeg(client, id, media.height, output, temp_dir).await,
        VRedditMode::Website(url) => website(client, &url.replacen("{}", id, 1), output).await,
    }
}

/// Downloads the video without audio.
async fn no_audio(client: &Client, url: &str, output: &Path) -> Result<()> {
    trace!("no_audio({}, {:?})", url, output);

    download(client, &url.parse()?, output).await?;

    Ok(())
}

/// Download video and audio, then merge them using `ffmpeg -y -i video -i audio output`.
async fn ffmpeg(
    client: &Client,
    id: &str,
    resolution: u64,
    output: &Path,
    temp_dir: &Path,
) -> Result<()> {
    trace!("ffmpeg({:?}, {:?})", id, output);

    let video_url = format!("https://v.redd.it/{}/DASH_{}", id, resolution).parse()?;
    let video_path = temp_dir.with_file_name(format!("v_redd_it_{}_video", id));
    let audio_url = format!("https://v.redd.it/{}/audio", id).parse()?;
    let audio_path = temp_dir.with_file_name(format!("v_redd_it_{}_audio", id));

    let video = download(client, &video_url, &video_path);
    let audio = download(client, &audio_url, &audio_path);

    let (video, audio) = futures_util::join!(video, audio);

    async fn clear(video_path: &Path, audio_path: &Path) {
        fs::remove_file(video_path).await;
        fs::remove_file(audio_path).await;
    }

    if video.is_err() && audio.is_err() {
        clear(&video_path, &audio_path).await;
        if let Err(e) = video {
            return Err(Error::new(format!(
                "Failed to combine audio and video: {}",
                e
            )));
        };
        if let Err(e) = audio {
            return Err(Error::new(format!(
                "Failed to combine audio and video: {}",
                e
            )));
        };
    };

    debug!("Generating file {:?} with `ffmpeg`", output);

    match Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(&video_path)
        .arg("-i")
        .arg(&audio_path)
        .arg("-c")
        .arg("copy")
        .arg(&output)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
    {
        Ok(status) => {
            if !status.success() {
                clear(&video_path, &audio_path).await;
                return Err(Error::new(format!(
                    "ffmpeg returned error status {}\n    Note: {}",
                    status, HELP_FFMPEG
                )));
            };
        }
        Err(e) => {
            clear(&video_path, &audio_path).await;
            if e.kind() == ErrorKind::NotFound {
                return Err(Error::new(format!("Failed to spawn ffmpeg command: {}\n    Note: If you are using '--vreddit-mode ffmpeg' you have to have a local copy of the program.", e)));
            } else {
                return Err(Error::new(format!("Failed to spawn ffmpeg command: {}", e)));
            };
        }
    };

    clear(&video_path, &audio_path).await;

    Ok(())
}

/// Use the URL to download the video.
async fn website(client: &Client, url: &str, output: &Path) -> Result<()> {
    trace!("website({:?}, {:?})", url, output);

    download(client, &url.parse()?, output).await
}
