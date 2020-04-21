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
Networking tools for the program.
*/

use std::path::Path;

use bytes::buf::BufExt;
use futures_util::stream::StreamExt;
pub use http::{request::Builder, Method, StatusCode, Uri};
pub use hyper::Body;
use hyper::{client::connect::HttpConnector, Response};
use hyper_tls::HttpsConnector;
use serde::de::DeserializeOwned;
use tokio::{fs::File, io::AsyncWriteExt};

use crate::prelude::*;

/// A client to perform HTTP requests with.
#[derive(Debug)]
pub struct Client(hyper::Client<HttpsConnector<HttpConnector>>);

impl Client {
    #[inline]
    pub fn new() -> Client {
        Client(hyper::Client::builder().build(HttpsConnector::new()))
    }

    /// Executes a HTTP request.
    /// The body can be read using [`to_disk()`] or [`to_json()`].
    ///
    /// Takes a `Result<...>` for convenience.
    ///
    /// [`to_disk()`]: fn.to_disk.html
    /// [`to_json()`]: fn.to_json.html
    pub async fn request(&self, request: Builder) -> Result<Response<Body>> {
        trace!("request({:?})", request);

        let request = request
            .header("Connection", "Close")
            .header("Accept-Encoding", "identity")
            .body(Body::empty())?;

        let response = self.0.request(request).await?;

        Ok(response)
    }
}

/// Parses a response as JSON.
pub async fn to_json<T: DeserializeOwned>(response: Response<Body>) -> Result<T> {
    trace!("to_json({:?})", response);

    let body = hyper::body::aggregate(response).await?;
    let value = serde_json::from_reader(body.reader())?;

    Ok(value)
}

/// Writes a response to the disk.
pub async fn to_disk(response: Response<Body>, output: &Path) -> Result<()> {
    trace!("to_disk({:?}, {:?})", response, output);

    let mut file = File::create(output).await?;
    let mut body = response.into_body();

    while let Some(i) = body.next().await {
        let i = i?;
        file.write_all(&i).await?;
    }

    Ok(())
}

/// Downloads a file.
pub async fn download(client: &Client, url: &Uri, output: &Path) -> Result<()> {
    trace!("download({:?}, {:?})", url, output);

    let response = client
        .request(Builder::new().method(Method::GET).uri(url))
        .await?;
    let status = response.status();

    if status.is_success() {
        debug!("Received {} from {:?}", status, url);
    } else if status.as_u16() == 404 {
        return Err(Error::new("File not found"));
    } else {
        return Err(Error::new(format!("Unexpected response code {}", status)));
    };

    to_disk(response, output).await?;

    Ok(())
}
