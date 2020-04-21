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
Utilities for retrieving data from the Pushshift API.
*/

use serde::Deserialize;

use crate::prelude::*;

/// The output of the Pushshift API.
#[derive(Deserialize, Debug)]
pub struct PushShift {
    data: Vec<Post>,
}

/// A post on reddit.
#[derive(Deserialize, Debug)]
pub struct Post {
    pub id: String,
    pub domain: String,
    pub url: String,
    pub title: String,
    pub created_utc: u64,
    pub secure_media: Option<SecureMedia>,
    pub selftext: Option<String>,
}

/// An optional part of a post on reddit.
#[derive(Deserialize, Debug)]
pub struct SecureMedia {
    pub reddit_video: Option<RedditVideo>,
}

/// A video hosted on `v.redd.it`.
#[derive(Deserialize, Debug)]
pub struct RedditVideo {
    /// The no-audio URL of the video.
    pub fallback_url: String,

    /// The video height.
    pub height: u64,
}

/// Retrieves data from the Pushshift API.
///
/// The `before` parameter is automatically set by the function:
/// the next call retrieves the next data. If the returned `Vec`
/// has a length of `0`, the available data was read completely.
///
/// The data is always returned from new to old.
pub async fn api(
    client: &Client,
    parameters: &Parameters,
    subreddit: &str,
    after: &Option<u64>,
    before: &mut Option<u64>,
) -> Result<Vec<Post>> {
    trace!("api({:?}, {:?}, {:?})", subreddit, after, before);

    let after_time = match after {
        Some(time) => format!("&after={}", time),
        None => String::new(),
    };
    let before_time = match before {
        Some(time) => format!("&before={}", time),
        None => String::new(),
    };

    let url = if parameters.selfposts {
        format!(
            "https://api.pushshift.io/reddit/search/submission?sort_type=created_utc&sort=desc&size={size:}&fields=created_utc,id,title,domain,url,secure_media,selftext&subreddit={subreddit:}{after:}{before:}",
            subreddit = subreddit,
            size = parameters.batch_size,
            after = after_time,
            before = before_time)
    } else {
        format!(
            "https://api.pushshift.io/reddit/search/submission?sort_type=created_utc&sort=desc&size={size:}&fields=created_utc,id,title,domain,url,secure_media&is_self=false&subreddit={subreddit:}{after:}{before:}",
            subreddit = subreddit,
            size = parameters.batch_size,
            after = after_time,
            before = before_time)
    };

    let response = client
        .request(
            Builder::new()
                .method(Method::GET)
                .uri(&url)
                .header("Accept", "application/json"),
        )
        .await?;

    if !response.status().is_success() {
        return Err(Error::new(format!(
            "Invalid response code {} from API",
            response.status()
        )));
    };

    debug!("Received {} from {:?}", response.status(), url);

    let value: PushShift = to_json(response).await?;
    let value = value.data;

    // Update the `before` parameter.
    // The next call automatically retrieves the next batch of data.
    // This is correct even when using `after` because the `sort_type` is set to `desc` (descending).
    if let Some(post) = value.last() {
        *before = Some(post.created_utc);
    };

    Ok(value)
}
