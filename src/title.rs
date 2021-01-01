/*
 * Copyright 2020 Draphar
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
Utilities for title formatting.
*/

use aho_corasick::AhoCorasick;
#[cfg(test)]
use serde_json::json;
use serde_json::Value;

/// The available fields.
pub static FIELDS: &'static [&'static str] = &[
    "test", // Used for testing purposes
    "allow_live_comments",
    "author",
    "author_flair_text",
    "author_fullname",
    "author_patreon_flair",
    "author_premium",
    "can_mod_post",
    "contest_mode",
    "created_utc",
    "crosspost_parent",
    "domain",
    "full_link",
    "id",
    "is_crosspostable",
    "is_meta",
    "is_original_content",
    "is_reddit_media_domain",
    "is_robot_indexable",
    "is_self",
    "is_video",
    "link_flair_background_color",
    "link_flair_text_color",
    "link_flair_text",
    "link_flair_type",
    "locked",
    "media_only",
    "no_follow",
    "num_comments",
    "num_crossposts",
    "over_18",
    "parent_whitelist_status",
    "permalink",
    "pinned",
    "post_hint",
    "pwls",
    "removed_by_category",
    "retrieved_on",
    "score",
    "selftext",
    "send_replies",
    "spoiler",
    "stickied",
    "subreddit",
    "subreddit_id",
    "subreddit_subscribers",
    "subreddit_type",
    "thumbnail",
    "title",
    "total_awards_received",
    "url",
    "whitelist_status",
    "wls",
];

/// A title formatter.
#[derive(Debug)]
pub struct Title {
    /// The formatting string.
    haystack: String,

    /// The fields that are present in the formatting string.
    fields: Vec<&'static str>,

    /// An iterator over the placeholders.
    formatter: AhoCorasick,
}

impl Title {
    /// Generates a formatter from a formatting string.
    pub fn new(haystack: &str) -> Title {
        let haystack = clean(haystack);
        let mut fields = Vec::new();
        let mut fields_placeholders = Vec::new();
        let patterns = FIELDS.iter().map(|field| format!("{{{}}}", field));

        for (i, pattern) in patterns.enumerate() {
            // Using the normal string searcher because constructing
            // an Aho-Corasick for only one search is too expensive
            if haystack.contains(&pattern) {
                fields.push(FIELDS[i]);
                fields_placeholders.push(pattern);
            };
        }

        Title {
            haystack,
            fields,
            formatter: AhoCorasick::new_auto_configured(&fields_placeholders),
        }
    }

    /// Returns whether the `{id}` placeholder is in the haystack.
    pub fn utilizes_id(&self) -> bool {
        self.fields.contains(&"id")
    }

    /// Returns an iterator over the fields.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.fields.iter().map(|item| *item)
    }

    /// Formats a title.
    /// The `json` parameter contains the replacement values.
    /// The `length` parameter describes the maximum allowed length.
    pub fn format(&self, json: &mut Value, length: usize) -> String {
        let mut buf = String::with_capacity(length);

        self.formatter
            .replace_all_with(&self.haystack, &mut buf, |i, _, buf| {
                let value = &json[self.fields[i.pattern()]];
                let text = if value.is_null() {
                    return true;
                } else if let Some(value) = value.as_str() {
                    clean(value)
                } else {
                    clean(&value.to_string())
                };
                buf.push_str(&text);

                true
            });

        // Todo: Deal with character boundaries
        buf.truncate(length);

        buf
    }
}

/// Replaces illegal characters in file names with `_`.
/// This method always writes exactly `title.len()` bytes.
fn clean(title: &str) -> String {
    let mut result = String::with_capacity(title.len());

    for i in title.chars() {
        result.push(match i {
            '/' | '\\' | '|' | '?' | '<' | '>' | ':' | '*' | '"' => '_',
            other => other,
        });
    }

    result
}

/// Returns a list of supported fields and their respective type.
pub fn formatting_help() -> &'static str {
    "\
allow_live_comments: bool
author: string
author_flair_text: string
author_fullname: string
author_patreon_flair: bool
author_premium: bool
can_mod_post: bool
contest_mode: bool
created_utc: integer
crosspost_parent: string
domain: string
full_link: string
id: string
is_crosspostable: bool
is_meta: bool
is_original_content: bool
is_reddit_media_domain: bool
is_robot_indexable: bool
is_self: bool
is_video: bool
link_flair_background_color: string
link_flair_text_color: string
link_flair_text: string
link_flair_type: string
locked: bool
media_only: bool
no_follow: bool
num_comments: integer
num_crossposts: integer
over_18: bool
parent_whitelist_status: string
permalink: string
pinned: bool
post_hint: string
pwls: integer
removed_by_category: string
retrieved_on: integer
score: integer
selftext: string
send_replies: bool
spoiler: bool
stickied: bool
subreddit: string
subreddit_id: string
subreddit_subscribers: integer
subreddit_type: string
thumbnail: string
title: string
total_awards_received: integer
url: string
whitelist_status: string
wls: integer
"
}

#[test]
fn format_no_fields() {
    let data = "Lorem ipsum";
    let fmt = Title::new(data);
    assert_eq!(data, fmt.format(&mut Value::Null, 0xf));

    let data = "Lorem ipsum";
    let fmt = Title::new(data);
    assert_eq!("L", fmt.format(&mut Value::Null, 1));
}

#[test]
fn format_overflowing_static() {
    let data = "1234 {test}";
    let fmt = Title::new(data);

    assert_eq!("12", fmt.format(&mut Value::Null, 2));
}

#[test]
fn format_null_field() {
    let data = "{test}";
    let fmt = Title::new(data);

    assert_eq!("", fmt.format(&mut Value::Null, 2));

    let data = "Lorem{test}ipsum";
    let fmt = Title::new(data);

    assert_eq!("Loremipsum", fmt.format(&mut Value::Null, 0xf));
}

#[test]
#[rustfmt::skip]
fn format_replace() {
    let data = "{test} ipsum";
    let fmt = Title::new(data);

    assert_eq!("Lorem ipsum", fmt.format(&mut json! {{
        "test": "Lorem"
    }}, 0xf));

    let data = "{test} ipsum {test}";
    let fmt = Title::new(data);

    assert_eq!("Lorem ipsum Lorem", fmt.format(&mut json! {{
        "test": "Lorem"
    }}, 0xff));

    let data = "{test} ipsum {id}";
    let fmt = Title::new(data);

    assert_eq!("Lorem ipsum dolor sit amet", fmt.format(&mut json! {{
        "test": "Lorem",
        "id": "dolor sit amet"
    }}, 0xff));
}

#[test]
#[rustfmt::skip]
fn format_clean() {
    let data = "Lorem/ipsum\\dolor|sit?amet,<consectetur>adipiscing:elit.*Vestibulum\"ut nisl.";
    let fmt = Title::new(data);

    assert_eq!(
        "Lorem_ipsum_dolor_sit_amet,_consectetur_adipiscing_elit._Vestibulum_ut nisl.",
        fmt.format(&mut Value::Null, 0xff)
    );

    let data = "Lorem {test}";
    let fmt = Title::new(data);

    assert_eq!("Lorem ______", fmt.format(&mut json! {{
        "test": "/\\|?<>"
    }}, 0xf));
}
