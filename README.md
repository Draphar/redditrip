# redditrip

[![crates.io-badge]][crates.io]

[crates.io-badge]: https://img.shields.io/crates/v/redditrip?style=flat-square 
[crates.io]: https://crates.io/crates/redditrip

A versatile tool for downloading the linked contents of entire subreddits fast and efficiently.

Note that this is not the tool for the job if you want to retrieve information about the individual reddit posts.

## Features

- bypass reddit API limitation of 1000 posts

- update local copy

- banned subreddits work

- fast (because of an asynchronous job queue, powered by the fantastic [`tokio`])

- custom title formatting

- (for the Rust programmers) `#![forbid(unsafe_code)]`

## Installation

Prebuilt binaries can be found in the [`Releases`] tab.

If you want a locally compiled version, you can use `cargo`:

1. Install Rust according to https://rustup.rs.
   The default configuration is sufficient.

2. `$ cargo install redditrip`

## Usage

The usage can also be acquired by running `redditrip --help`. 

The base command is `redditrip [SUBREDDITS]...`.  
The following arguments can optionally be used:

- `--update`/`-u`: Stop at the first already existing file.

- `--force`/`-f`: Force downloads from unsupported domains by simpling writing whatever is on the page to disk.

- `--after <date>`: Only download posts after this date.

- `--before <date>`: Only download posts before this date.

- `--selfposts`/`-s`: Download self posts as text files.

- `--allow <domain>`/`--exclude <domain>`: Allows only or prevents downloading from a domain, respectively. Multiple values are supported.

- `--title <formatter>`: Use a custom title format.

There are a couple of more advanced options described in the `--help` output.

#### Downloading large amounts of data

It is recommended to use `-q`/`--quiet` to see only the individual errors.
You should also use a high `--batch-size`, and subsequently a high `ulimit -n` (open files) because every download job takes >= 1 open file descriptor.
Finally, if you expect to run into a lot of unsupported sites, which can directly be saved, use `--force`.

#### Title formatting

`redditrip` supports custom titles. To use this feature, a formatting string must be provided with `--title <formatter>`.
Placeholders are the field names enclosed in curly brackets. For example: `--title "{id}-{author}_{title}"`.
The file extension is always appended to the title.

The available fields can be queried by running the program with `--formatting-fields`.
They correspond to data from the Pushshift API.
An rough overview can be seen on https://api.pushshift.io/reddit/search/submission?size=1, though not all fields can be present.

Characters which are not allowed in file names are replaced with `_`.

Because reddit titles alone can be longer than the maximum file name length on many systems, one should know about `--max-file-name-length <length>`,
which is used to truncate file names.

Not using the `{id}` placeholder can lead to file name collisions, thus it is advised to always include it somewhere in the formatter,
preferably at the front so it is not at the risk of being truncated.

The most useful placeholders are:

| Placeholder | Type | Purpose |
| :---------: | ---- | ------- |
| `{id}` | string | The post ID |
| `{title}` | string | The post title |
| `{author}` | string | The post author name |
| `{created_utc}` | integer | The UNIX timestamp when the post was created |
| `{link_flair_text}` | string | The text of the post flair |
| `{author_flair_text}` | string | The text of the author flair |
| `{domain}` | string | The domain of the link the post points to |
| `{over_18}` | bool | Whether the post is NSFW |

## Compiling

It's quite easy to compile the master branch yourself.
Make sure that Rust and `cargo` are installed like described above.
Then:

```
$ git clone https://github.com/Draphar/redditrip
$ cd redditrip
```

Then this is sufficient:

```
$ cargo build --release
  # The binary is `target/release/redditrip`
```

If you, for whatever reason, want a highly optimized build:

```
$ cargo rustc --release -- -C lto -C codegen-units=1 -C target_cpu=native
$ strip target/release/redditrip
```

## Todo

- implement a map of already downloaded links, and symlink instead of redownloading

- filter posts

### License

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.

[`Releases`]: https://github.com/Draphar/redditrip/releases
[`tokio`]: https://tokio.rs/
