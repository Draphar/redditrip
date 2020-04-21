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

use std::{collections::HashSet, env, ffi::OsString, future::Future, path::Path, process};

use futures_util::stream::{FuturesUnordered, StreamExt};
use http::Uri;
use tokio::fs;

use crate::prelude::*;
use crate::sites::{fetch, file_extension, pushshift, FetchJob};

/// Initiates the subreddit download.
pub async fn rip(parameters: Parameters, subreddits: Vec<String>) -> Result<()> {
    trace!("rip({:?}, {:?})", parameters, subreddits);

    let client = Client::new();
    let mut temp_dir = env::temp_dir();
    let mut queue = FuturesUnordered::new();

    temp_dir.push("index"); // overwritten later by `with_file_name()`

    'subreddit_loop: for subreddit in subreddits {
        let after = parameters.after;
        let mut before = parameters.before;
        let mut output = parameters.output.to_owned();
        output.push(&subreddit);
        if let Err(e) = fs::create_dir_all(&output).await {
            error!("Failed to create directory: {}", e);
            process::exit(1);
        };
        let file_names = get_file_names(&output).await;
        let self_domain = format!("self.{}", subreddit);

        info!("Started ripping /r/{} to {:?}", subreddit, output);

        output.push("index"); // overwritten later by `with_file_name()`

        loop {
            let data =
                pushshift::api(&client, &parameters, &subreddit, &after, &mut before).await?;

            if data.is_empty() {
                continue 'subreddit_loop;
            };

            debug!("Read {} posts from /r/{:?}", data.len(), subreddit);

            for mut i in data {
                let domain = i.domain;
                let url = match i.url.parse::<Uri>() {
                    Ok(value) => value,
                    // This will probably never happen because reddit checks URLs
                    Err(e) => {
                        warn!("Invalid URL {:?}: {}", i.url, e);
                        continue;
                    }
                };
                let extension = file_extension(&url, parameters.gfycat_type, domain == self_domain);
                // The space required for the ID and file extension
                let required_len = i.id.len() + 1 + extension.unwrap_or("").len();
                // Truncate the title if the length is too long
                i.title
                    .truncate(parameters.max_file_name_length - required_len);

                let mut file_name = String::with_capacity(required_len + i.title.len());
                file_name += &i.id;
                file_name.push('-');
                clean_title(&i.title, &mut file_name);
                if let Some(extension) = extension {
                    file_name += extension;
                };

                if parameters.update
                    && file_names.contains(AsRef::<std::ffi::OsStr>::as_ref(file_name.as_str()))
                {
                    info!("File already exists: {:?}", file_name);
                    continue 'subreddit_loop;
                };

                queue.push(fetch(FetchJob {
                    client: &client,
                    parameters: &parameters,
                    is_selfpost: domain == self_domain,
                    domain,
                    url,
                    output: output.with_file_name(file_name),
                    temp_dir: &temp_dir,
                    text: i.selftext,
                    media: i.secure_media,
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
            Ok(()) => info!("Saved {:?}", job.output.file_name().unwrap()),
            Err(e) => warn!("Failed to retrieve {}:\n    {}", job.url, e),
        };
    }
}

/// Gets a set of file names in the directory.
async fn get_file_names(directory: &Path) -> HashSet<OsString> {
    let mut file_names = HashSet::new();
    let mut stream = fs::read_dir(directory).await.unwrap();

    while let Some(entry) = stream.next().await {
        if let Ok(entry) = entry {
            file_names.insert(entry.file_name());
        };
    }

    file_names
}

/// Replaces illegal characters for file names with `_`.
/// This method always writes exactly `title.len()` bytes.
fn clean_title(title: &str, output: &mut String) {
    for i in title.chars() {
        output.push(match i {
            '/' | '?' | '<' | '>' | '\\' | ':' | '*' | '"' => '_',
            other => other,
        });
    }
}