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
Support for [Imgur](https://imgur.com/) downloads.

# Domains

- `i.imgur.com`
- `imgur.com`
*/

use std::{io::BufRead, path::Path};

use bytes::buf::BufExt;
use http::Uri;
use serde::Deserialize;
use serde_json::Value;
use tokio::fs;

use crate::prelude::*;

/// Fetches an image from `i.imgur.com`.
pub async fn fetch(client: &Client, url: &Uri, output: &Path) -> Result<()> {
    trace!("fetch({:?}, {:?})", url, output);

    let response = client.request(Builder::new().uri(url.clone())).await?;
    let status = response.status();

    if status.is_success() {
        debug!("Received {} from {:?}", status, url);
    } else if status.as_u16() == 302 {
        // Imgur redirects to `imgur.com/*` instead of a normal 404.
        return Err(Error::new("File not found"));
    } else {
        return Err(Error::new(format!("Unexpected response code {}", status)));
    };

    to_disk(response, output).await?;

    Ok(())
}

/// An image on Imgur.
#[derive(Deserialize, Debug, Eq, PartialEq)]
struct Image {
    hash: String,
    ext: String,
}

/// Fetches Imgur albums and galleries.
pub async fn fetch_album(client: &Client, url: &Uri, output: &Path) -> Result<()> {
    if url.path().starts_with("/a/") {
        download_images(client, album(client, url, output).await?, output).await
    } else if url.path().starts_with("/gallery/") {
        let mut id = url.path();
        // Remove trailing `/`
        if id.ends_with('/') {
            id = &id[..id.len() - 1];
        };

        download_images(client, gallery(client, &id[9..], output).await?, output).await
    } else {
        // Just assume that a direct link was used without the
        // `i.` prefix. An `imgur.com/*` link redirects to
        // `i.imgur.com/*`, so directly download from there.
        debug!("Trying to directly download image {}", url);
        fetch(
            client,
            &format!("https://i.imgur.com{}", url.path())
                .parse()
                .unwrap(),
            output,
        )
        .await
    }
}

/// Fetches an album using a HTML scraper.
async fn album(client: &Client, url: &Uri, output: &Path) -> Result<Vec<Image>> {
    trace!("album({:?}, {:?})", url, output);

    let slash = &url.path()[3..].find('/').map(|n| n + 3);
    let id = &url.path()[3..slash.unwrap_or_else(|| url.path().len())];
    let url = format!("https://imgur.com/a/{}/embed", id);

    let response = client
        .request(Builder::new().method(Method::GET).uri(&url))
        .await?;
    let status = response.status();

    if status.is_success() {
        debug!("Received {} from {:?}", status, url);
    } else if status.as_u16() == 404 {
        return Err(Error::new("File not found"));
    } else {
        return Err(Error::new(format!("Unexpected response code {}", status)));
    };

    let lines = hyper::body::aggregate(response).await?.reader().lines();

    for i in lines {
        let i = i.unwrap(); // Because the contents of `impl Buf` are in memory, this operation is infallible (see `bytes` documentation)

        if i.trim_start().starts_with("album") {
            // This line contains the JSON.
            let colon = i
                .find(':')
                .ok_or_else(|| Error::new("Imgur parser error"))?;
            let end = i.trim_end().len();
            let mut json: Value = serde_json::from_str(&i[(colon + 1)..(end - 1)])?;
            let images = serde_json::from_value(json["album_images"]["images"].take())?;
            return Ok(images);
        };
    }

    Err(Error::new("Imgur parser error"))
}

/// Extracts the images from a gallery using a JSON API.
async fn gallery(client: &Client, id: &str, output: &Path) -> Result<Vec<Image>> {
    trace!("gallery({:?}, {:?})", id, output);

    let url = format!("https://imgur.com/gallery/{}.json", id);
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

    let mut json: Value = to_json(response).await?;
    let images = serde_json::from_value(json["data"]["image"]["album_images"]["images"].take())?;

    Ok(images)
}

/// Downloads the set of images.
async fn download_images(client: &Client, images: Vec<Image>, output: &Path) -> Result<()> {
    trace!("download_images({:?}, {:?})", images, output);

    debug!("Found Imgur gallery containing {} entries", images.len());

    fs::create_dir_all(output).await?;
    let mut path = output.to_path_buf();
    path.push("index"); // later overwritten
    for (i, image) in images.into_iter().enumerate() {
        let path = path.with_file_name(format!("{}{}", i, image.ext));
        debug!("Saving individual image \"{}{}\"", image.hash, image.ext);
        download(
            client,
            &format!("https://i.imgur.com/{}{}", image.hash, image.ext).parse()?,
            &path,
        )
        .await; // ignore individual errors
    }

    // Todo: A future join could be of use here.

    Ok(())
}

#[tokio::test]
#[cfg_attr(not(feature = "__tests-network"), ignore)]
async fn imgur_album() {
    let client = Client::new();
    let images = album(
        &client,
        &"https://imgur.com/a/dFz23".parse().unwrap(),
        "".as_ref(),
    )
    .await
    .unwrap();
    assert_eq!(
        vec![
            Image {
                hash: "bxv008g".to_string(),
                ext: ".gif".to_string()
            },
            Image {
                hash: "oXx9m52".to_string(),
                ext: ".gif".to_string()
            },
            Image {
                hash: "s3XOVHt".to_string(),
                ext: ".png".to_string()
            },
            Image {
                hash: "EanxY6r".to_string(),
                ext: ".gif".to_string()
            }
        ],
        images
    );
}

#[tokio::test]
#[cfg_attr(not(feature = "__tests-network"), ignore)]
async fn imgur_gallery() {
    let client = Client::new();
    let images = gallery(&client, "dFz23", "".as_ref()).await.unwrap();
    assert_eq!(
        vec![
            Image {
                hash: "bxv008g".to_string(),
                ext: ".gif".to_string()
            },
            Image {
                hash: "oXx9m52".to_string(),
                ext: ".gif".to_string()
            },
            Image {
                hash: "s3XOVHt".to_string(),
                ext: ".png".to_string()
            },
            Image {
                hash: "EanxY6r".to_string(),
                ext: ".gif".to_string()
            }
        ],
        images
    );
}
