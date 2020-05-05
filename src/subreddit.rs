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
Fetches posts from a subreddit.
*/

use std::{env, fs, future::Future, io::ErrorKind, path::Path, process};

use futures_util::stream::{FuturesUnordered, StreamExt};
use http::Uri;
use tokio::io;

use crate::logger::{color_stderr, color_stdout};
use crate::prelude::*;
use crate::sites::{
    fetch, FetchJob,
    file_extension,
    pushshift::{self, Subreddit},
};

const UPDATE_FILE_NAME: &'static str = ".redditrip";

/// Initiates the subreddit download.
pub async fn rip(parameters: Parameters, subreddits: Vec<Subreddit>) -> Result<()> {
    trace!("rip({:?}, {:?})", parameters, subreddits);

    let client = Client::new();
    let mut temp_dir = env::temp_dir();
    let mut queue = FuturesUnordered::new();
    let api_url = pushshift::build_api_url(&parameters);

    temp_dir.push("index"); // overwritten later by `with_file_name()`

    'subreddit_loop: for subreddit in subreddits {
        let subreddit_name = subreddit.to_string();
        let mut before = parameters.before;
        let mut updated = false;
        let api_url = format!(
            "{}{}",
            api_url,
            match &subreddit {
                Subreddit::Subreddit(name) => format!("&subreddit={}", name),
                Subreddit::Profile(name) => format!("&author={}", name),
            }
        );

        let mut output = parameters.output.to_owned();
        output.push(subreddit.to_path());
        if let Err(e) = fs::create_dir_all(&output) {
            error!("Failed to create directory: {}", e);
            process::exit(1);
        };

        output.push("index"); // overwritten later by `with_file_name()`

        // The ID of the newest file in the directory
        let newest_id = match read_update_file(&output) {
            Ok(value) => Some(value),
            Err(ref e) if e.kind() == ErrorKind::NotFound => None,
            Err(e) => {
                warn!(
                    "Failed to open the update file `.redditrip`, even though it is present: {}",
                    e
                );
                None
            }
        };

        info!(
            "Started ripping {} to {}",
            color_stdout(&subreddit_name),
            color_stdout(&output.parent().unwrap().display())
        );

        loop {
            let data = pushshift::api(&client, &api_url, &mut before).await?;

            if data.is_empty() {
                continue 'subreddit_loop;
            };

            debug!("Read {} posts from {}", data.len(), subreddit_name);

            for mut i in data {
                if let Some(id) = i["id"].as_str() {
                    if parameters.update && Some(id) == newest_id.as_ref().map(|s| s.as_str()) {
                        info!("Post {} already exists", color_stdout(&id));
                        run_jobs(&mut queue).await;
                        continue 'subreddit_loop;
                    };

                    if !updated {
                        if let Err(e) = create_update_file(&output, id).await {
                            warn!("Failed to create update file `{}`: {}\n    Using the '--update' argument will not work", UPDATE_FILE_NAME, e);
                        } else {
                            debug!("Created update file `{}`", UPDATE_FILE_NAME);
                        };
                        updated = true;
                    };
                } else {
                    warn!("Malformed JSON response");
                    continue;
                };

                let url = if let Some(url) = i["url"].as_str() {
                    match url.parse::<Uri>() {
                        Ok(value) => value,
                        Err(e) => {
                            warn!("Invalid URL {}: {}", color_stderr(&url), e);
                            continue;
                        }
                    }
                } else {
                    warn!("Malformed JSON response");
                    continue;
                };
                let is_self = if let Some(value) = i["is_self"].as_bool() {
                    value
                } else {
                    warn!("Malformed JSON response");
                    continue;
                };
                let extension = file_extension(&url, parameters.gfycat_type, is_self).unwrap_or("");

                let mut title = parameters
                    .title
                    .format(&mut i, parameters.max_file_name_length - extension.len());
                title.push_str(extension);

                let post: pushshift::Post = match serde_json::from_value(i) {
                    Ok(value) => value,
                    Err(e) => {
                        warn!("Malformed JSON response: {}", e);
                        continue;
                    }
                };

                queue.push(fetch(FetchJob {
                    client: &client,
                    parameters: &parameters,
                    is_selfpost: is_self,
                    domain: post.domain,
                    url,
                    output: output.with_file_name(title),
                    temp_dir: &temp_dir,
                    text: post.selftext,
                    media: post.secure_media,
                }));
            }

            run_jobs(&mut queue).await;
        }
    }

    Ok(())
}

/// Runs the job queue to completion.
async fn run_jobs(queue: &mut FuturesUnordered<impl Future<Output = (FetchJob<'_>, Result<()>)>>) {
    while let Some((job, result)) = queue.next().await {
        match result {
            Ok(()) => info!(
                "Saved {}",
                color_stdout(&Path::new(job.output.file_name().unwrap()).display())
            ),
            Err(e) => warn!("Failed to retrieve {}:\n    {}", color_stderr(&job.url), e),
        };
    }
}

/// Returns the most recent post ID from a marker file in the directory.
fn read_update_file(directory: &Path) -> io::Result<String> {
    let file = directory.with_file_name(UPDATE_FILE_NAME);
    let mut data = fs::read_to_string(&file)?;
    let line = if let Some(index) = data.find('\n') {
        data.truncate(index);
        data
    } else {
        data
    };

    Ok(line)
}

/// Creates a new update containing the content.
async fn create_update_file(directory: &Path, content: &str) -> io::Result<()> {
    let file = directory.with_file_name(UPDATE_FILE_NAME);
    let mut content = content.as_bytes().to_vec();
    content.extend_from_slice(b"\n# This is a file generated by redditrip to keep track of the already downloaded files.\n# Modify at your own risk!");
    tokio::fs::write(&file, content).await
}

#[tokio::test]
#[allow(unused_must_use)]
async fn update_file() {
    let mut directory = env::temp_dir();
    directory.push("index");
    {
        create_update_file(&directory, "Lorem").await.unwrap();
        create_update_file(&directory, "ipsum").await.unwrap();
        create_update_file(&directory, "dolor").await.unwrap();
    };
    assert_eq!("dolor", read_update_file(&directory).unwrap());

    fs::remove_file(directory.with_file_name(UPDATE_FILE_NAME));
}
