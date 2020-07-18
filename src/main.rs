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
A versatile tool for downloading the linked contents of entire subreddits fast and efficiently.

# Usage

`redditrip [FLAGS] [OPTIONS] [SUBREDDITS]...`

## Flags

- `-q, --quiet`
 Disable output on stdout

- `-v`, `--verbose`
 Enable verbose output

- `--domains`
 Output a list of supported domains

- `-f`, `--force`
 Whether to force the download from unsupported domains by simpling writing whatever is on the page to disk.

- `--formatting-fields`
 Display the possible placeholders for the '--title' argument. Note that not all fields are set for every post.

- `--no-parent`
 Normally, a directory is created as a subdirectory of '--output'. This option causes the files to be placed directly within '--output'.

- `-s`, `--selfposts`
 Download self posts as text files

- `-u`, `--update`
 Stop at the first already existing file for each subreddit. If this flag is not given, everything is overwritten if it exists.

## Options

- `--after <date>`
 Only download posts after this date. The date should be formatted like 'YYYY-MM-DD', with an optionally appended time in the format 'HH:MM:SS', or a UNIX timestamp with second precision.

-- `allow <domain>`
 Only allows downloading from a domain. It is practical to use brace expansion syntax for this argument: `--allow={"i.redd.it","i.imgur.com"}`.

- `--before <date>`
 Only download posts before this date. The date should be formatted like 'YYYY-MM-DD', with an optionally appended time in the format 'HH:MM:SS', or a UNIX timestamp with second precision.

 `-b`, `--queue-size <size>`
 A number between 1 and 1000 that specifies the number of simultaneous download jobs. A higher number eats more resources, but is faster. [default: 16]

- `-C`, `--color <'auto'|'always'|'never'>`
 Enable colored output [default: auto]  [possible values: always, auto, never]

- `-e`, `--exclude <domain>`
 Prevents downloading from a domain. It is practical to use brace expansion syntax for this argument: `--exclude={"i.redd.it","i.imgur.com"}`.

- `--gfycat-type <type>`
 The media type of gfycat videos [default: mp4]  [possible values: mp4, webm]

- `--max-file-name-length <length>`
 Some systems impose restrictions to file names. If you run into a "File name too long" error, look up what the maximum allowed length on your system is and pass it with this parameter. The value of this argument is in bytes, not characters. [default: 255]

- `-o, --output <directory>`
 The output directory [default: .]

- `-t`, `--title <title>`
 This argument takes a string containing placeholders which are replaced with the values of each respective post. All possible placeholders can be retrieved by running the program with '--formatting-fields'. The placeholders are enclosed in curly braces. For example: '--title "{author}_{title}-{created_utc}"'. Note that not all fields are set for every post. Unset placeholder values are replaced by an empty string. Also note that the formatted string is always followed by the file extension, if any. The file name length  is also limited on most file systems. The '--max-file-name-length' argument is used to truncate the generated name. It is moreover advised to include `{id}` in the title to prevent collisions. [default: {id}-{title}]

- `--vreddit-mode <mode>`
 This setting specifies how videos are downloaded from `v.redd.it`. The value 'no-audio' downloads videos without audio. The value 'ffmpeg' downloads video and audio separately and combines them using the `ffmpeg` command, which must be installed locally. Any other value must be a valid URL, in which the string `{}` is replaced by the video ID, that is the part after that comes after `v.redd.it/` in URLs. [default: no-audio]

# Exit status

- `0` if the program was able to run and download at least one post;
      and only minor errors occurred during the execution

- `1` if an initial error occurred and the program was not able to start

- `2` if a crucial network error occurred

- `3` if an unexpected error occurred which normally indicates
      that one the APIs and services used is broken

*/

#![forbid(unsafe_code)]

extern crate aho_corasick;
extern crate ansi_term; // already required by structopt
extern crate atty; // already required by structopt
extern crate bytes; // already required by hyper
extern crate futures_util; // already required by hyper
extern crate http; // already required by hyper
extern crate hyper;
extern crate hyper_tls; // already required by hyper
#[macro_use]
extern crate log;
extern crate serde; // already required by serde_json
extern crate serde_json;
extern crate structopt;
extern crate time;
extern crate tokio; // already required by hyper

use std::{
    fmt::Display,
    io::{stdin, ErrorKind},
    mem,
    path::PathBuf,
    process::{self, Command, Stdio},
    str::FromStr,
};

use ansi_term::Color;
use atty::Stream;
use http::uri::Uri;
use structopt::StructOpt;
use time::{strftime, strptime, Timespec};
use tokio::runtime::Builder;

use crate::error::{HELP_JSON, HELP_NETWORK};
use crate::sites::{gfycat::GfycatType, pushshift::Subreddit, reddit::VRedditMode};
use crate::title::Title;
use logger::color_stdout;

mod error;
mod logger;
mod net;
mod sites;
mod subreddit;
mod title;

mod prelude {
    pub use crate::error::*;
    pub use crate::net::*;
    pub use crate::Parameters;
}

/// The command line arguments.
///
/// Generated by `structopt`.
#[derive(Debug, StructOpt)]
#[structopt(
    name = "redditrip",
    author = "Made by Joshua Prieth, licensed under the Apache-2.0 license.",
    long_about = "\
        A versatile tool for downloading the linked contents of entire subreddits fast and efficiently. \
        Run `cargo install redditrip --force` to update. \
    "
)]
// When changing any of the default values, also edit the `test_build_api_url()` test
pub struct Parameters {
    #[structopt(short, long, conflicts_with("quiet"), help = "Enable verbose output")]
    verbose: bool,

    #[structopt(short, long, help = "Disable output on stdout")]
    quiet: bool,

    #[structopt(long, hidden = true, requires = "verbose", conflicts_with("quiet"))]
    very_verbose: bool,

    #[structopt(
        short = "C", long, possible_values = &["always", "auto", "never"], default_value = "auto", value_name = "'auto'|'always'|'never'",
        help = "Enable colored output"
    )]
    color: String,

    #[structopt(long, help = "Output a list of supported domains")]
    domains: bool,

    #[structopt(
        long,
        value_name = "length",
        default_value = "255",
        help = "The maximum file name length in bytes",
        long_help = "\
            Some systems impose restrictions to file names. If you run \
            into a \"File name too long\" error, look up what the maximum \
            allowed length on your system is and pass it with this parameter. \
            The value of this argument is in bytes, not characters.\
        "
    )]
    max_file_name_length: usize,

    #[structopt(
        short,
        long,
        parse(from_os_str),
        value_name = "directory",
        default_value = ".",
        help = "The output directory"
    )]
    output: PathBuf,

    #[structopt(
        short,
        long,
        help = "Force downloads from unknown domains",
        long_help = "\
            Whether to force the download from unsupported domains \
            by simpling writing whatever is on the page to disk.\
        "
    )]
    force: bool,

    #[structopt(
        short,
        long,
        help = "Update the local copy",
        long_help = "\
            Stop at the first already existing file for each subreddit. \
            If this flag is not given, everything is overwritten if it exists.\
        "
    )]
    update: bool,

    #[structopt(
        long,
        help = "Do not create a subdirectory",
        long_help = "\
            Normally, a directory is created as a subdirectory of '--output'. \
            This option causes the files to be placed directly within '--output'. \
        "
    )]
    no_parent: bool,

    #[structopt(
        long, parse(try_from_str = parse_date), value_name = "date",
        help = "Filter for posts after this date",
        long_help = "\
            Only download posts after this date. The date should be formatted like \
            'YYYY-MM-DD', with an optionally appended time in the format 'HH:MM:SS', \
            or a UNIX timestamp with second precision.\
        "
    )]
    after: Option<u64>,

    #[structopt(
        long, parse(try_from_str = parse_date), value_name = "date",
        help = "Filter for posts before this date",
        long_help = "\
            Only download posts before this date. The date should be formatted like \
            'YYYY-MM-DD', with an optionally appended time in the format 'HH:MM:SS', \
            or a UNIX timestamp with second precision.\
        "
    )]
    before: Option<u64>,

    #[structopt(
        long,
        short = "b",
        default_value = "16",
        value_name = "size",
        alias = "batch-size",
        help = "The number of simultaneous downloads",
        long_help = "\
            A number between 1 and 1000 that specifies the number of simultaneous \
            download jobs. A higher number eats more resources, but is faster. \
        "
    )]
    queue_size: usize,

    #[structopt(
        name = "SUBREDDITS", parse(try_from_str = parse_input),
        help = "A list of subreddits or profiles to download",
        long_help = "\
            The input subreddits or profiles. Unless prefixed with 'u/' or '/u/', \
            it is assumed that the input is a subreddit.
        "
    )]
    subreddits: Vec<Subreddit>,

    #[structopt(short, long, help = "Download self posts as text files")]
    selfposts: bool,

    #[structopt(
        long, parse(try_from_str = parse_domains), multiple = true, value_name = "domain", conflicts_with("exclude"),
        help = "Only download from the domain",
        long_help = "\
            Only allows downloading from a domain. It is practical to use brace \
            expansion syntax for this argument: '--allow={\"i.redd.it\",\"i.imgur.com\"}'.\
        "
    )]
    allow: Option<Vec<String>>,

    #[structopt(
        short, long, parse(try_from_str = parse_domains), multiple = true, value_name = "domain",
        help = "Do not download from the domain",
        long_help = "\
            Prevents downloading from a domain. It is practical to use brace \
            expansion syntax for this argument: '--exclude={\"i.redd.it\",\"i.imgur.com\"}'.\
        "
    )]
    exclude: Option<Vec<String>>,

    #[structopt(
        long, parse(from_str), possible_values = &["mp4", "webm"], default_value = "mp4", value_name = "type",
        help = "The media type of gfycat videos"
    )]
    gfycat_type: GfycatType,

    #[structopt(
        long,
        parse(from_str),
        default_value = "no-audio",
        value_name = "mode",
        help = "Set the v.redd.it mode",
        long_help = "\
            This setting specifies how videos are downloaded from `v.redd.it`. \
            The value 'no-audio' downloads videos without audio. The value \
            'ffmpeg' downloads video and audio separately and combines them using \
            the `ffmpeg` command, which must be installed locally. Any other value \
            must be a valid URL, in which the string `{}` is replaced by the video \
            ID, that is the part after that comes after `v.redd.it/` in URLs.\
        "
    )]
    vreddit_mode: VRedditMode,

    #[structopt(
        long,
        help = "Display the available formatting fields",
        long_help = "\
            Display the possible placeholders for the '--title' argument. Note \
            that not all fields are set for every post.\
        "
    )]
    formatting_fields: bool,

    #[structopt(
        short, long, parse(from_str = Title::new), default_value = "{id}-{title}",
        help = "Use a custom title format",
        long_help = "\
            This argument takes a string containing placeholders which \
            are replaced with the values of each respective post. All \
            possible placeholders can be retrieved by running the program \
            with '--formatting-fields'. The placeholders are enclosed \
            in curly braces. For example: '--title \"{author}_{title}-\
            {created_utc}\"'. Note that not all fields are set for every \
            post. Unset placeholder values are replaced by an empty string.
\
            Also note that the formatted string is always followed by the \
            file extension, if any. The file name length  is also limited \
            on most file systems. The '--max-file-name-length' argument \
            is used to truncate the generated name. It is moreover \
            advised to include `{id}` in the title to prevent collisions.\
        "
    )]
    title: Title,
}

/// Parses a subreddit name.
///
/// The input is assumed to be a subreddit unless prefixed with `u/` or `/u/`.
/// The prefixes `r/`, `/r/`, `u/` and `/u/` are automatically removed.
/// An error is returned if the name is invalid.
fn parse_input(name: &str) -> Result<Subreddit, String> {
    if name.starts_with("/u/") {
        return verify_name(&name[3..]).map(|_| Subreddit::Profile(name[3..].to_string()));
    };
    if name.starts_with("u/") {
        return verify_name(&name[2..]).map(|_| Subreddit::Profile(name[2..].to_string()));
    };
    if name.starts_with("/r/") {
        return verify_name(&name[3..]).map(|_| Subreddit::Subreddit(name[3..].to_string()));
    } else if name.starts_with("r/") {
        return verify_name(&name[2..]).map(|_| Subreddit::Subreddit(name[2..].to_string()));
    };

    verify_name(&name)?;

    Ok(Subreddit::Subreddit(name.to_string()))
}

/// Verifies a subreddit name.
fn verify_name(name: &str) -> Result<(), String> {
    if name.len() > 21 {
        return Err(String::from(
            "Subreddit names have a maximum length of 21 characters",
        ));
    };

    for i in name.chars() {
        if !i.is_alphanumeric() && i != '-' && i != '_' {
            return Err(format!(
                "Subreddit names can only contain alphanumeric characters, '{}' found",
                i
            ));
        };
    }

    Ok(())
}

/// Parses a date.
///
/// The available formats are
///
/// - "YYYY-MM-DD HH:MM:SS"
/// - "YYYY-MM-DD"
/// - `any valid u64` as a UNIX timestamp, second precision
///
/// in that order.
fn parse_date(input: &str) -> Result<u64, &'static str> {
    strptime(input, "%F %T")
        .or_else(|_| strptime(input, "%F"))
        .map(|time| time.to_timespec().sec as u64)
        .or_else(|_| u64::from_str(input))
        .map_err(|_| "Invalid date format")
}

/// Parses an input and returns the domain.
/// This function automatically detects URL-like input and extracts the host.
fn parse_domains(input: &str) -> Result<String, String> {
    // Parse the input as URI to make it more ergonomic
    Uri::from_str(input)
        .map_err(|e| format!("{}", e))
        .and_then(|uri| {
            uri.host()
                .map(|uri| uri.to_owned())
                .ok_or_else(|| String::from("No domain found"))
        })
}

/// Parses the command line arguments and runs the tool.
fn main() {
    let mut parameters = Parameters::from_args();

    if parameters.domains {
        println!("{}", sites::supported_domains());
        return;
    };

    if parameters.formatting_fields {
        print!("{}", title::formatting_help());
        return;
    };

    let colors = match parameters.color.as_ref() {
        "always" => (true, true),
        "never" => (false, false),
        _ => {
            let mut stdout = false;
            let mut stderr = false;

            if atty::is(Stream::Stdout) {
                stdout = true;
            };
            if atty::is(Stream::Stderr) {
                stderr = true;
            };

            (stdout, stderr)
        }
    };

    let verbosity = if parameters.verbose {
        if parameters.very_verbose {
            5
        } else {
            4
        }
    } else if parameters.quiet {
        2
    } else {
        3
    };

    logger::init(verbosity, colors.0, colors.1);

    if parameters.subreddits.is_empty() {
        info!("No input subreddit given");
        return;
    };

    if !parameters.title.utilizes_id() {
        let warn: Box<dyn Display> = if cfg!(not(windows)) && colors.0 {
            Box::new(Color::Yellow.paint("[WARN]"))
        } else {
            Box::new("[WARN]")
        };
        println!("{}    The title formatting string does not contain `{{id}}`. File name collisions may occur.", warn);
    };

    for i in parameters.subreddits.iter() {
        if let Subreddit::Subreddit(i) = i {
            if !i.is_empty() {
                continue;
            };

            let warn: Box<dyn Display> = if cfg!(not(windows)) && colors.0 {
                Box::new(Color::Yellow.paint("[WARN]"))
            } else {
                Box::new("[WARN]")
            };
            println!("{}    An empty argument was passed, the result will be that the entirety of reddit will be downloaded. Do you want to continue?\n[Y/n]", warn);
            let mut buf = String::new();
            stdin().read_line(&mut buf).unwrap();
            let input = buf.to_lowercase();
            if !(input == "y\n" || input == "yes\n" || input == "\n") {
                return;
            };
        };
    }

    // Check whether ffmpeg is installed
    if let VRedditMode::Ffmpeg = parameters.vreddit_mode {
        if let Err(e) = Command::new("ffmpeg")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
        {
            if e.kind() == ErrorKind::NotFound {
                error!("'--vreddit-mode ffmpeg' set, but ffmpeg is not installed\n\nPlease make sure that you have ffmpeg installed and it is in your path variable.");
                process::exit(1);
            } else {
                warn!("Failed to start ffmpeg: {}\n\nNote: this is not an error, but you should make sure that ffmpeg is properly available", e);
            };
        };
    };

    fn format_time(time: u64) -> String {
        let sec = time as i64;
        strftime("%c", &time::at_utc(Timespec { sec, nsec: 0 })).unwrap()
    }

    if parameters.after.is_some() && parameters.before.is_some() {
        info!(
            "Downloading posts between {} and {}",
            color_stdout(&format_time(parameters.after.unwrap())),
            color_stdout(&format_time(parameters.before.unwrap()))
        );
    } else if let Some(time) = parameters.after {
        info!(
            "Downloading posts after {}",
            color_stdout(&format_time(time))
        );
    } else if let Some(time) = parameters.before {
        info!(
            "Downloading posts before {}",
            color_stdout(&format_time(time))
        );
    };

    let subreddits = mem::replace(&mut parameters.subreddits, Vec::new());

    match Builder::new().threaded_scheduler().enable_all().build() {
        Ok(mut runtime) => {
            if let Err(e) = runtime.block_on(subreddit::rip(parameters, subreddits)) {
                if e.source().is_none() {
                    error!("Error: {}", e);
                    process::exit(3);
                };
                let e = e.into_source().unwrap();

                let e = match e.downcast::<hyper::Error>() {
                    Ok(e) => {
                        if e.is_connect() {
                            error!("Essential HTTP request failed: {}\n\n{}", e, HELP_NETWORK);
                        } else {
                            error!("Essential HTTP request failed: {}", e);
                        };
                        process::exit(2);
                    }
                    Err(e) => e,
                };

                let e = match e.downcast::<serde_json::Error>() {
                    Ok(e) => {
                        error!("Unexpectedly received invalid JSON: {}\n\n{}", e, HELP_JSON);
                        process::exit(3);
                    }
                    Err(e) => e,
                };

                error!("Error: {}", e);
                process::exit(3);
            }
        }
        Err(e) => {
            error!("Failed to start runtime: {}\n\n{}", e, error::HELP_BUG);
            process::exit(1);
        }
    };
}
