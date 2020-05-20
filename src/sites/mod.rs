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
Download support for the individual sites.
*/

use std::path::{Path, PathBuf};

use http::Uri;
use tokio::{fs::File, io::AsyncWriteExt};

use gfycat::GfycatType;

use crate::prelude::*;
use crate::sites::pushshift::SecureMedia;

pub mod gfycat;
pub mod imgur;
pub mod pinterest;
pub mod postimages;
pub mod pushshift;
pub mod reddit;

/// A fetching job.
/// Used for describing every download job.
#[derive(Debug)]
pub struct FetchJob<'a> {
    /// The HTTP client to use.
    pub client: &'a Client,

    /// The parameters passed to the program.
    pub parameters: &'a Parameters,

    /// The domain of the post.
    pub domain: String,

    /// Whether the post is a self post.
    pub is_selfpost: bool,

    /// The URL the post links to.
    /// This is not necessarily a reddit URL.
    pub url: Uri,

    /// The output file.
    pub output: PathBuf,

    /// The directory for temporary files.
    /// Used only while processing with `ffmpeg`.
    pub temp_dir: &'a Path,

    /// The text of the post if it is a self post.
    pub text: Option<String>,

    /// The `secure_media` property if the item is a `v.redd.it` video.
    pub media: Option<SecureMedia>,
}

/// Runs the fetch job.
pub async fn fetch(config: FetchJob<'_>) -> (FetchJob<'_>, Result<()>) {
    trace!("fetch({:?})", config.url);

    let result = if config.is_selfpost {
        debug!("Detected self post {:?}", config.url);

        if let Some(text) = config.text.as_ref() {
            fetch_selfpost(&config.output, text).await
        } else {
            // Seriously reddit?
            return (
                config,
                Err(Error::new("Malformed self post: field 'selftext' missing")),
            );
        }
    } else {
        debug!("Fetching {:?}", config.url);

        match config.domain.as_ref() {
            "i.redd.it" => reddit::fetch_image(config.client, &config.url, &config.output).await,
            "v.redd.it" => {
                reddit::fetch_video(
                    config.client,
                    &config.url,
                    &config.output,
                    &config.temp_dir,
                    &config.parameters.vreddit_mode,
                    &config.media,
                )
                .await
            }
            "i.imgur.com" => imgur::fetch(config.client, &config.url, &config.output).await,
            "imgur.com" => imgur::fetch_album(config.client, &config.url, &config.output).await,
            "gfycat.com" => {
                gfycat::fetch_gfycat(
                    config.client,
                    &config.url,
                    &config.output,
                    config.parameters.gfycat_type,
                )
                .await
            }
            "redgifs.com" => {
                gfycat::fetch_redgifs(
                    config.client,
                    &config.url,
                    &config.output,
                    config.parameters.gfycat_type,
                )
                .await
            }
            "giant.gfycat.com" => {
                gfycat::fetch_giant(config.client, &config.url, &config.output).await
            }
            "thumbs.gfycat.com" | "thumbs1.redgifs.com" => {
                gfycat::fetch_thumbs(config.client, &config.url, &config.output).await
            }
            "i.pinimg.com" => pinterest::fetch(config.client, &config.url, &config.output).await,
            "i.postimg.cc" => postimages::fetch(config.client, &config.url, &config.output).await,
            domain => {
                if config.parameters.force {
                    download(config.client, &config.url, &config.output).await
                } else {
                    Err(Error::new(format!("Unsupported domain '{}'", domain)))
                }
            }
        }
    };

    (config, result)
}

/// Fetches a self post.
pub async fn fetch_selfpost(output: &PathBuf, text: &str) -> Result<()> {
    trace!("fetch_selfpost({:?}, {:?})", output, text);

    let mut file = File::create(&output).await?;
    file.write_all(text.as_bytes()).await?;

    Ok(())
}

/// Gets the file extension of an URL.
pub fn file_extension(url: &Uri, gfycat_type: GfycatType, is_selfpost: bool) -> Option<&str> {
    if is_selfpost {
        return Some(".txt");
    };

    if url.host() == Some("v.redd.it") {
        return Some(".mp4");
    };

    if url.host() == Some("gfycat.com") {
        return match gfycat_type {
            GfycatType::Mp4 => Some(".mp4"),
            GfycatType::Webm => Some(".webm"),
        };
    };

    let mut chars = url.path().char_indices();

    while let Some((index, c)) = chars.next_back() {
        if c == '.' {
            return Some(&url.path()[index..]);
        } else if c == '/' {
            // Abort at the first slash
            return None;
        };
    }

    None
}

/// Returns the currently supported domains.
pub fn supported_domains() -> &'static str {
    "\
i.redd.it
v.redd.it
i.imgur.com
imgur.com
gfycat.com
thumbs.gfycat.com
giant.gfycat.com
redgifs.com
thumbs1.redgifs.com
i.pinimg.com
i.postimg.cc\
    "
}

#[test]
fn test_url_extension() {
    let data = "http://example.com/";
    assert_eq!(
        Some(".txt"),
        file_extension(&Uri::from_static(data), GfycatType::Mp4, true)
    );

    let data = "http://example.com/a/b.c";
    assert_eq!(
        Some(".c"),
        file_extension(&Uri::from_static(data), GfycatType::Mp4, false)
    );
    let data = "http://example.com/a.bc";
    assert_eq!(
        Some(".bc"),
        file_extension(&Uri::from_static(data), GfycatType::Mp4, false)
    );

    let data = "http://example.com/";
    assert_eq!(
        None,
        file_extension(&Uri::from_static(data), GfycatType::Mp4, false)
    );
    let data = "http://example.com/none";
    assert_eq!(
        None,
        file_extension(&Uri::from_static(data), GfycatType::Mp4, false)
    );

    let data = "https://gfycat.com/";
    assert_eq!(
        Some(".mp4"),
        file_extension(&Uri::from_static(data), GfycatType::Mp4, false)
    );
    let data = "https://gfycat.com/";
    assert_eq!(
        Some(".webm"),
        file_extension(&Uri::from_static(data), GfycatType::Webm, false)
    );
    let data = "http://gfycat.com/.webm";
    assert_eq!(
        Some(".mp4"),
        file_extension(&Uri::from_static(data), GfycatType::Mp4, false)
    );
    let data = "http://gfycat.com/.mp4";
    assert_eq!(
        Some(".webm"),
        file_extension(&Uri::from_static(data), GfycatType::Webm, false)
    );

    let data = "http://imgur.com/image.jpg";
    assert_eq!(
        Some(".jpg"),
        file_extension(&Uri::from_static(data), GfycatType::Mp4, false)
    );
    let data = "http://imgur.com/a/id";
    assert_eq!(
        None,
        file_extension(&Uri::from_static(data), GfycatType::Mp4, false)
    );
    let data = "http://imgur.com/a/id/";
    assert_eq!(
        None,
        file_extension(&Uri::from_static(data), GfycatType::Mp4, false)
    );
}
