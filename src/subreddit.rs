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

use std::{collections::HashSet, env, future::Future, path::Path, process};

use futures_util::stream::{FuturesUnordered, StreamExt};
use http::Uri;
use tokio::fs;

use crate::prelude::*;
use crate::sites::{
    fetch, file_extension,
    pushshift::{self, Subreddit},
    FetchJob,
};

/// Initiates the subreddit download.
pub async fn rip(parameters: Parameters, subreddits: Vec<Subreddit>) -> Result<()> {
    trace!("rip({:?}, {:?})", parameters, subreddits);

    let client = Client::new();
    let mut temp_dir = env::temp_dir();
    let mut queue = FuturesUnordered::new();

    temp_dir.push("index"); // overwritten later by `with_file_name()`

    'subreddit_loop: for subreddit in subreddits {
        let subreddit_name = subreddit.to_string();
        let after = parameters.after;
        let mut before = parameters.before;
        let mut output = parameters.output.to_owned();
        output.push(subreddit.to_path());
        if let Err(e) = fs::create_dir_all(&output).await {
            error!("Failed to create directory: {}", e);
            process::exit(1);
        };
        let post_ids = match if parameters.update {
            get_post_ids(&output).await
        } else {
            Ok(HashSet::new())
        } {
            Ok(set) => set,
            Err(e) => {
                warn!("Failed to read contents of {:?}: {}", output, e);
                HashSet::new()
            }
        };

        info!("Started ripping {} to {:?}", subreddit_name, output);

        output.push("index"); // overwritten later by `with_file_name()`

        loop {
            let data =
                pushshift::api(&client, &parameters, &subreddit, &after, &mut before).await?;

            if data.is_empty() {
                continue 'subreddit_loop;
            };

            debug!("Read {} posts from {}", data.len(), subreddit_name);

            for mut i in data {
                let domain = i.domain;

                if parameters.exclude.contains(&domain) {
                    info!("Skipped {}", i.url);
                    continue;
                };

                let url = match i.url.parse::<Uri>() {
                    Ok(value) => value,
                    // This will probably never happen because reddit checks URLs
                    Err(e) => {
                        warn!("Invalid URL {:?}: {}", i.url, e);
                        continue;
                    }
                };
                let extension = file_extension(&url, parameters.gfycat_type, i.is_self);
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

                if parameters.update && post_ids.contains(&i.id) {
                    info!("File already exists: {:?}", file_name);
                    continue 'subreddit_loop;
                };

                queue.push(fetch(FetchJob {
                    client: &client,
                    parameters: &parameters,
                    is_selfpost: i.is_self,
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

/// Gets a set of item IDs in the directory.
/// Only detects file names in the format `{id}-*`.
async fn get_post_ids(directory: &Path) -> Result<HashSet<String>> {
    let mut post_ids = HashSet::new();
    let mut stream = fs::read_dir(directory).await?;

    while let Some(entry) = stream.next().await {
        // Assume that the program generates only valid UTF-8
        if let Some(mut entry) = entry
            .ok()
            .map(|a| a.file_name())
            .and_then(|name| name.into_string().ok())
        {
            // Extract the ID from the file name
            if let Some(index) = entry.find('-') {
                entry.truncate(index);
                post_ids.insert(entry);
            };
        };
    }

    Ok(post_ids)
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
