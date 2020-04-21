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

There are a couple of more advanced options described in the `--help` output.

#### Downloading large amounts of data

It is recommended to use `-q`/`--quiet` to see only the individual errors.
You should also use a high `--batch-size`, and subsequently a high `ulimit -n` (open files) because every download job takes >= 1 open file descriptor.
Finally, if you expect to run into a lot of unsupported sites, which can directly be saved, use `--force`.

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

- support user profiles

- implement a map of already downloaded links, and symlink instead of redownloading

- exclude domains

- filter posts

### License

Copyright 2020 Joshua Prieth

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
