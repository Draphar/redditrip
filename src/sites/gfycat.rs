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
Support for [Gfycat](https://gfycat.com/) downloads.

# Domains

- `gfycat.com`
- `thumbs.gfycat.com`
- `giant.gfycat.com`
*/

use std::path::Path;

use http::Uri;
use serde::*;

use crate::prelude::*;

/// Specifies the format videos from Gfycat are downloaded in.
#[derive(Debug, Clone, Copy)]
pub enum GfycatType {
    /// Use `mp4` videos.
    Mp4,

    /// Use `webm` videos.
    Webm,
}

impl<'a> From<&'a str> for GfycatType {
    fn from(s: &str) -> Self {
        match s {
            "mp4" => GfycatType::Mp4,
            "webm" => GfycatType::Webm,
            _ => unreachable!(), // Guaranteed by clap's `possible_values`
        }
    }
}

/// Information about a Gfycat video.
#[derive(Deserialize)]
#[allow(non_snake_case)]
struct Gfycat {
    gfyItem: GfyItem,
}

/// A Gfycat video.
#[derive(Deserialize)]
#[allow(non_snake_case)]
struct GfyItem {
    mp4Url: String,
    webmUrl: String,
}

/// Fetches a video from `gfycat.com`.
pub async fn fetch(
    client: &Client,
    url: &Uri,
    output: &Path,
    gfycat_type: GfycatType,
) -> Result<()> {
    trace!("fetch({:?}, {:?}, {:?})", url, output, gfycat_type);

    let (id, well_formed) = extract_id(url);

    // If the ID seems to be well-formed, use it directly.
    if well_formed {
        debug!("Trying to download directly from Gfycat {}", id);

        if fetch_giant(
            client,
            &format!(
                "https://giant.gfycat.com/{}.{}",
                id,
                match gfycat_type {
                    GfycatType::Mp4 => "mp4",
                    GfycatType::Webm => "webm",
                }
            )
            .parse()?,
            output,
        )
        .await
        .is_ok()
        {
            return Ok(());
        };
    };

    api(client, id, output, gfycat_type).await
}

/// Extracts the Gfycat ID from the URL.
fn extract_id(url: &Uri) -> (&str, bool) {
    // Gfycat URLs a fascinating thing. They occur
    // as all-lowercase, well-formed, and with
    // the title appended in the wild. This part
    // tries to extract the Gfycat-ID, which can
    // be used to retrieve the video directly
    // without an API call if it is well-formed.

    // Get the part between the initial `/` and the first `-`, if any.
    let id = if let Some(index) = url.path().chars().position(|c| c == '-') {
        &url.path()[1..index]
    } else {
        &url.path()[1..]
    };

    (id, id.chars().any(|c| c.is_ascii_uppercase()))
}

/// Fetches a video from `giant.gfycat.com`.
pub async fn fetch_giant(client: &Client, url: &Uri, output: &Path) -> Result<()> {
    trace!("fetch_giant({:?}, {:?})", url, output);

    download(client, url, output).await
}

/// Fetches a video from `thumbs.gfycat.com`.
pub async fn fetch_thumbs(client: &Client, url: &Uri, output: &Path) -> Result<()> {
    trace!("fetch_thumbs({:?}, {:?})", url, output);

    download(client, url, output).await
}

/// Use the Gfycat API to retrieve the download link.
/// Rate limits may be encountered because an API
/// key is required to thoroughly use the API.
async fn api(client: &Client, id: &str, output: &Path, gfycat_type: GfycatType) -> Result<()> {
    trace!("api({:?}, {:?}, {:?})", id, output, gfycat_type);
    debug!("Querying Gfycat api about {}", id);

    let mut url = String::from("https://api.gfycat.com/v1/gfycats/");
    url += id;

    let response = client
        .request(
            Builder::new()
                .method(Method::GET)
                .uri(&url)
                .header("Accept", "application/json"),
        )
        .await?;
    let status = response.status();

    if status.is_success() {
        debug!("Received {} from {:?}", status, url);
    } else if status.as_u16() == 404 {
        return Err(Error::new("File not found"));
    } else {
        return Err(Error::new(format!("Unexpected response code {}", status)));
    };

    let gfycat: Gfycat = to_json(response).await?;

    let url = match gfycat_type {
        GfycatType::Mp4 => gfycat.gfyItem.mp4Url,
        GfycatType::Webm => gfycat.gfyItem.webmUrl,
    };

    fetch_giant(client, &url.parse()?, output).await?;

    Ok(())
}

#[test]
fn gfycat_id() {
    assert_eq!(
        ("loremipsum", false),
        extract_id(&"https://gfycat.com/loremipsum".parse().unwrap())
    );
    assert_eq!(
        ("LoremIpsum", true),
        extract_id(&"https://gfycat.com/LoremIpsum".parse().unwrap())
    );
    assert_eq!(
        ("loremipsum", false),
        extract_id(&"https://gfycat.com/loremipsum-some-text".parse().unwrap())
    );
    assert_eq!(
        ("LoremIpsum", true),
        extract_id(&"https://gfycat.com/LoremIpsum-some-text".parse().unwrap())
    );
}
