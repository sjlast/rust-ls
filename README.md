# rust-ls

A macOS-oriented Rust implementation of `ls`. The project is being built as a learning
project and aims to support common macOS directory-listing workflows.

> **Status:** Early development. The command supports basic directory enumeration, short option
> parsing, long-format metadata, and readable sizes. Alternate sorting and recursive traversal
> are still being implemented.

## Build and run

Requires Rust 1.85 or later.

```sh
cargo build
cargo run -- [OPTIONS] [PATH...]
```

With no paths, the command lists the current directory. Supported options are:

| Option | Intended behavior                                                      |
| ------ | ---------------------------------------------------------------------- |
| `-a`   | Include `.` and `..` as well as hidden entries                         |
| `-A`   | Include hidden entries, excluding `.` and `..`                         |
| `-l`   | Show file type, permissions, links, owner, group, size, time, and name |
| `-h`   | Use readable sizes with `-l`                                           |
| `-1`   | Print one entry per line (the default prints entries on one line)      |
| `-t`   | Sort by modification time, newest first                                |
| `-S`   | Sort by size, largest first                                            |
| `-r`   | Reverse the selected ordering                                          |
| `-R`   | Recursively list subdirectories                                        |

Short options can be combined (for example, `-al`), and `--` ends option parsing so
dash-prefixed paths can be used as operands. See
[`specs/PLAN.md`](specs/PLAN.md) for the detailed behavior
and acceptance criteria.

## Library interface

The crate also exposes listing types and a `list` function for callers that need a reusable
interface:

```rust
use rust_ls::{list, ListingOptions};

let result = list(&ListingOptions::default(), &[]);
```

The result keeps selected entries and filesystem errors separate. This interface is under
development along with the command-line behavior.

## Development

Run the test suite with:

```sh
cargo test
```

The project targets macOS conventions and is not intended to provide full GNU `ls`
compatibility.
