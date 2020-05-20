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
Support for [Gfycat](https://gfycat.com/) and [Redgifs](https://redgifs.com/) downloads.

# Domains

- `gfycat.com`
- `thumbs.gfycat.com`
- `giant.gfycat.com`
- `redgifs.com`
- `thumbs1.redgifs.com`
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

impl GfycatType {
    #[inline]
    pub fn as_str(self) -> &'static str {
        match self {
            GfycatType::Mp4 => "mp4",
            GfycatType::Webm => "webm",
        }
    }
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
pub async fn fetch_gfycat(
    client: &Client,
    url: &Uri,
    output: &Path,
    gfycat_type: GfycatType,
) -> Result<()> {
    trace!("fetch({:?}, {:?}, {:?})", url, output, gfycat_type);

    let (id, well_formed) = extract_id(url.path());

    // If the ID seems to be well-formed, use it directly.
    if well_formed {
        debug!("Trying to download directly from Gfycat {}", id);

        let url = format!("https://giant.gfycat.com/{}.{}", id, gfycat_type.as_str());

        if fetch_giant(client, &url.parse()?, output).await.is_ok() {
            return Ok(());
        };
    };

    let mut url = String::from("https://api.gfycat.com/v1/gfycats/");
    url += id;

    api(client, &url, output, gfycat_type).await
}

/// Fetches a video from `redgifs.com`.
pub async fn fetch_redgifs(
    client: &Client,
    url: &Uri,
    output: &Path,
    gfycat_type: GfycatType,
) -> Result<()> {
    trace!("fetch({:?}, {:?}, {:?})", url, output, gfycat_type);

    let (id, well_formed) = extract_id(
        url.path()
            .get(6..) // Cut off the `/watch`
            .ok_or_else(|| Error::new("Malformed URL"))?,
    );

    // If the ID seems to be well-formed, use it directly.
    if well_formed {
        debug!("Trying to download directly from Redgifs {}", id);

        let url = format!(
            "https://thumbs1.redgifs.com/{}.{}",
            id,
            gfycat_type.as_str()
        );

        if fetch_giant(client, &url.parse()?, output).await.is_ok() {
            return Ok(());
        };
    };

    let mut url = String::from("https://api.redgifs.com/v1/gfycats/");
    url += id;

    api(client, &url, output, gfycat_type).await
}

/// Extracts the Gfycat ID from the URL.
fn extract_id(url: &str) -> (&str, bool) {
    // Gfycat URLs a fascinating thing. They occur
    // as all-lowercase, well-formed, and with
    // the title appended in the wild. This part
    // tries to extract the Gfycat-ID, which can
    // be used to retrieve the video directly
    // without an API call if it is well-formed.

    // Get the part between the initial `/` and the first `-`, if any.
    let id = if let Some(index) = url.chars().position(|c| c == '-') {
        &url[1..index]
    } else {
        &url[1..]
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
async fn api(client: &Client, url: &str, output: &Path, gfycat_type: GfycatType) -> Result<()> {
    trace!("api({:?}, {:?}, {:?})", url, output, gfycat_type);
    debug!("Querying Gfycat api about {}", url);

    let response = client
        .request(
            Builder::new()
                .method(Method::GET)
                .uri(url)
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
    assert_eq!(("loremipsum", false), extract_id("/loremipsum"));
    assert_eq!(("LoremIpsum", true), extract_id("/LoremIpsum"));
    assert_eq!(("loremipsum", false), extract_id("/loremipsum-some-text"));
    assert_eq!(("LoremIpsum", true), extract_id("/LoremIpsum-some-text"));
}
