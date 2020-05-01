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
use serde_json::Value;

use crate::prelude::*;

/// A subreddit on reddit.
///
/// It might seem surprising that the profiles are summarised under a structure called "subreddit",
/// however reddit actually treats user profiles as subreddits: `/r/u_example` is the same as `/u/example`,
/// and when posting to one's profile one is really posting to `/r/u_{username}`.
#[derive(Debug, PartialEq)]
pub enum Subreddit {
    /// A subreddit.
    Subreddit(String),

    /// The profile of a user.
    Profile(String),
}

impl Subreddit {
    /// Converts this subreddit into a string usable as a path.
    pub fn to_path(&self) -> String {
        match self {
            Subreddit::Subreddit(name) => name.to_owned(),
            Subreddit::Profile(name) => format!("u_{}", name),
        }
    }
}

impl ToString for Subreddit {
    fn to_string(&self) -> String {
        match self {
            Subreddit::Subreddit(name) => format!("/r/{}", name),
            Subreddit::Profile(name) => format!("/u/{}", name),
        }
    }
}

/// The output of the Pushshift API.
#[derive(Deserialize, Debug)]
pub struct PushShift {
    data: Vec<Post>,
}

/// A post on reddit.
#[derive(Deserialize, Debug)]
pub struct Post {
    pub domain: String,
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

/// Creates an URL for the Pushshift API which can later be reused.
pub fn build_api_url(parameters: &Parameters) -> String {
    format!(
        "https://api.pushshift.io/reddit/search/submission?sort_type=created_utc&sort=desc&size={size:}&fields={fields:}{selfposts:}{domains:}{after:}",
        size = parameters.batch_size,
        fields = {
            let mut fields = String::from("id,created_utc,domain,url,secure_media,is_self");
            for i in parameters.title.iter() {
                fields.push(',');
                fields.push_str(i);
            };
            fields
        },
        selfposts = if parameters.selfposts {
            ",selftext"
        } else {
            "&is_self=false"
        },
        domains = if parameters.exclude.is_empty() {
            String::new()
        } else {
            parameters.exclude.iter().enumerate().fold(String::from("&domain="), |mut accumulator,(i, domain)| {
                if i != 0 {
                    accumulator.push(',');
                };
                accumulator.push('!');
                accumulator.push_str(domain);

                accumulator
            })
        },
        after = match parameters.after {
            Some(time) => format!("&after={}", time),
            None => String::new(),
        }
    )
}

/// Retrieves data from the Pushshift API.
///
/// The `before` parameter is automatically set by the function:
/// the next call retrieves the next data. If the returned `Vec`
/// has a length of `0`, the available data was read completely.
///
/// The data is always returned from new to old.
pub async fn api(client: &Client, url: &str, before: &mut Option<u64>) -> Result<Vec<Value>> {
    trace!("api({:?}, {:?})", url, before);

    let mut url = url.to_owned();
    url.push_str(&match before {
        Some(time) => format!("&before={}", time),
        None => String::new(),
    });

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

    let err = || {
        Error::new(format!(
            "Unexpectedly received invalid JSON\n\n{}",
            HELP_JSON
        ))
    };
    let mut value: Value = to_json(response).await?;
    if let Value::Array(posts) = value["data"].take() {
        // Update the `before` parameter.
        // The next call automatically retrieves the next batch of data.
        // This is correct even when using `after` because the `sort_type` is set to `desc` (descending).
        if let Some(post) = posts.last() {
            *before = Some(post["created_utc"].as_u64().ok_or_else(err)?);
        };

        Ok(posts)
    } else {
        Err(err())
    }
}

#[test]
fn test_build_api_url() {
    use structopt::StructOpt;

    assert_eq!(
        "https://api.pushshift.io/reddit/search/submission?sort_type=created_utc&sort=desc&size=250&fields=id,created_utc,domain,url,secure_media,is_self,id,title&is_self=false",
        build_api_url(&Parameters::from_iter(&["test"]))
    );
    assert_eq!(
        "https://api.pushshift.io/reddit/search/submission?sort_type=created_utc&sort=desc&size=0&fields=id,created_utc,domain,url,secure_media,is_self,id,title,selftext",
        build_api_url(&Parameters::from_iter(&["test", "--batch-size", "0", "--selfposts"]))
    );
    assert_eq!(
        "https://api.pushshift.io/reddit/search/submission?sort_type=created_utc&sort=desc&size=250&fields=id,created_utc,domain,url,secure_media,is_self,id,title&is_self=false&domain=!domain1,!domain2",
        build_api_url(&Parameters::from_iter(&["test", "--exclude", "domain1", "--exclude", "domain2"]))
    );
    assert_eq!(
        "https://api.pushshift.io/reddit/search/submission?sort_type=created_utc&sort=desc&size=250&fields=id,created_utc,domain,url,secure_media,is_self,id,title&is_self=false&after=946684800",
        build_api_url(&Parameters::from_iter(&["test", "--after", "2000-1-1"]))
    );
    assert_eq!(
        "https://api.pushshift.io/reddit/search/submission?sort_type=created_utc&sort=desc&size=250&fields=id,created_utc,domain,url,secure_media,is_self,author,full_link,id&is_self=false",
        build_api_url(&Parameters::from_iter(&["test", "--title", "{id}{author}{full_link}"]))
    );
}
