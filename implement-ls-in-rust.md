# Rust `ls`

## Problem Statement

Users need a Rust implementation of `ls` that behaves predictably on Linux and macOS. It should cover the directory listing workflows people commonly use from a shell, including hidden entries, detailed metadata, alternate sorting, and recursive traversal. The current project contains no implementation, so the supported behavior and completion criteria need to be defined before development.

## Solution

Build a command-line program in Rust that lists files and directories using familiar `ls` conventions. Support the options and behaviors defined in this specification, while documenting platform-dependent metadata. The first version supports `-a`, `-A`, `-l`, `-h`, `-R`, `-r`, `-t`, `-S`, and `-1`. It supports both direct command-line use and a reusable listing interface whose behavior can be tested independently.

## User Stories

1. As a shell user, I want to list the current directory when I run the program without paths, so that I can quickly see its contents.
2. As a shell user, I want to list one or more explicitly named files, so that I can inspect paths without listing their parent directories.
3. As a shell user, I want to list one or more directories, so that I can inspect their contents.
4. As a shell user, I want directory entries sorted by name by default, so that listings are predictable.
5. As a shell user, I want names beginning with `.` hidden by default, so that the default output stays focused on ordinary entries.
6. As a shell user, I want `-a` to include all entries, including `.` and `..`, so that I can inspect the complete directory entry set.
7. As a shell user, I want `-A` to include hidden entries while excluding `.` and `..`, so that I can see hidden files without navigation entries.
8. As a shell user, I want `-1` to print one entry per line, so that output is easy to read and process in a terminal or script.
9. As a shell user, I want `-l` to show a long listing with file type and permissions, link count, owner, group, size, modification time, and name, so that I can inspect file metadata.
10. As a shell user, I want `-h` with `-l` to display file sizes in readable units, so that large sizes are easier to compare.
11. As a shell user, I want `-t` to sort by modification time, newest first, so that recently changed entries appear first.
12. As a shell user, I want `-S` to sort by file size, largest first, so that I can find large files quickly.
13. As a shell user, I want `-r` to reverse the selected ordering, so that I can see entries in the opposite order.
14. As a shell user, I want `-R` to list directories recursively, so that I can inspect a directory tree in one command.
15. As a shell user, I want recursive output to identify each directory being listed, so that entries from different levels are distinguishable.
16. As a shell user, I want a missing path to produce a useful error and a nonzero exit status, so that I can identify invalid input in a terminal or script.
17. As a shell user, I want an inaccessible path to report an error without terminating listings for other valid paths, so that one failure does not hide other results.
18. As a shell user, I want invalid options to be reported clearly with a nonzero exit status, so that I can correct the command.
19. As a shell user, I want filenames containing spaces and non-ASCII characters displayed as names, so that ordinary filesystem names remain usable.
20. As a developer, I want to exercise listing behavior through a reusable interface, so that core behavior can be tested without relying on terminal formatting.
21. As a developer, I want to exercise the built command-line program, so that argument parsing, output, errors, and exit status are checked as users experience them.

## Implementation Decisions

- Support Linux and macOS. Use standard filesystem metadata available on each platform; document differences where owner, group, permissions, or timestamps cannot be represented identically.
- With no path arguments, list the current directory. Explicit file arguments are displayed as entries; explicit directory arguments list their immediate contents unless `-d` is later added (directory-as-entry behavior is out of scope for this version). An explicitly supplied symlink to a directory is followed for enumeration. File operands are sorted together as one group using the selected sort order; directory operands are processed in their original argument order, and each directory's contents form a separate listing.
- By default, omit dot-prefixed entries and sort names in ascending, case-sensitive lexical order. `-a` includes `.` and `..`; because directory enumeration does not return these entries, synthesize them for each directory listing and obtain their metadata from the listed directory and its parent. `-A` includes other hidden entries but omits `.` and `..`.
- `-1` selects one entry per line. Long format `-l` prints one entry per line and includes type/permissions, link count, owner, group, size, modification time, and name. `-h` changes the size display in long format to readable units; without `-l`, it has no visible effect.
- `-t` orders entries by modification time, newest first. `-S` orders entries by size, largest first. When neither is supplied, order by name. If both `-t` and `-S` are supplied, `-S` takes precedence regardless of option order. Entries tied by the selected sort key use ascending name order before reversal. `-r` reverses the complete selected ordering, including its name-based tie order.
- Use non-following metadata for entries discovered while listing a directory, including symlink entries. A displayed symlink's size is the size of its link text; `-S` sorts by the same size shown by `-l`. When an explicitly supplied symlink to a directory is followed for enumeration, list the target directory's entries. During `-R`, display symlinks to directories but do not descend through them.
- `-R` descends into subdirectories and prints a directory heading before each directory's entries. Report traversal errors and continue with other reachable entries.
- Combined short options (such as `-al`) are accepted. `--` ends option parsing; all following arguments are operands, including names beginning with `-`. An unsupported option produces a diagnostic and a nonzero exit status.
- Each invalid or inaccessible input path produces a diagnostic on standard error. Continue processing other operands; return nonzero if any operand or recursive traversal failed, and zero if all requested work succeeded.
- Preserve filename bytes internally. In output, leave valid Unicode and spaces unchanged, encode each backslash as `\\`, and encode control characters and bytes that are not valid UTF-8 as `\xNN` using two hexadecimal digits. This makes escaped bytes distinguishable from literal backslashes.
- Keep a reusable listing behavior interface for core enumeration, filtering, sorting, and metadata selection. It accepts parsed options and operands and returns entries plus per-path or per-entry errors; it does not render diagnostics or determine the process exit status. The CLI parses arguments, renders entries and diagnostics, continues after individual errors where possible, and returns a nonzero status if any operation failed. Specific module names and dependencies are implementation choices.

## Testing Decisions

- Test externally visible behavior rather than private implementation details. Use temporary directory trees with controlled filenames, metadata where practical, and permissions where the platform permits reliable permission tests.
- Test the reusable listing interface for default filtering and sorting, synthesized `.` and `..` under `-a`, `-A` filtering, time and size order, ties and reversal, long-format fields, readable sizes, symlink sizes, and recursive traversal without following directory symlinks.
- Test the executable end to end for default invocation, file and directory operands, explicit symlink-to-directory operands, combined short options, `--` with dash-prefixed names, one-entry-per-line output, long format, errors on standard error, continuation across valid and invalid operands, and exit statuses.
- Cover filenames with spaces, non-ASCII characters, control characters, and invalid UTF-8 bytes where the platform permits creating them; verify the specified escaping. Avoid asserting platform-specific owner names, permission encodings, or timestamp text beyond the documented contract.
- Test file-operand grouping and order, directory-operand order, and stable name tie-breaking for each selected sort key. Verify that displayed symlinks use link metadata and are not traversed when discovered during recursion.
- Compare semantic output and ordering in tests; avoid brittle assertions on terminal width or color because neither is part of this version.
- No existing test infrastructure or similar tests are present in the repository. Establish the smallest suitable unit and process-level test setup as part of implementation.

## Out of Scope

- Full GNU `ls` compatibility or all GNU/BSD options; compatibility is limited to the options and behaviors defined above.
- Colorized output, terminal-width-aware columns, and terminal capability detection.
- Directory-as-entry mode (`-d`), color indicators, inode/block counts, and configurable time formats.
- Sorting by extension, version, locale collation, or access/change time.
- Following symbolic links discovered during recursive traversal.
- Windows support.

## Further Notes

- The repository currently contains only project guidance and this specification; there is no Rust crate or implementation to preserve.
- Tracker publishing and the `ready-for-agent` triage label are part of the `to-spec` workflow, but tracker configuration and label vocabulary were not provided. The skill instructions say to run `/setup-matt-pocock-skills` when that information is missing.

## Tasks

### T1: Establish the Rust command and reusable listing interface

Create the executable and a testable listing boundary. The listing interface accepts parsed options and operands and returns entries plus per-path or per-entry errors; the CLI renders diagnostics and determines the final status.

### T2: Implement operand handling, filtering, and default ordering

List the current directory by default, support file and directory operands, omit hidden entries by default, implement `-a` and `-A`, and sort by name.

### T3: Implement option parsing and line-oriented output

Accept the specified combined short options, `--`, and `-1`, with clear diagnostics and failure status for unsupported options.

### T4: Implement long format and readable sizes

Render the specified metadata for `-l` and readable sizes for `-h` when used with long format, accounting for documented platform differences.

### T5: Implement sorting options

Add modification-time sorting with `-t`, size sorting with `-S`, `-S` precedence when both are set, name tie-breaking, and reversal with `-r`.

### T6: Implement recursive traversal and error continuation

Add `-R`, directory headings, no-follow behavior for discovered symlinked directories, and continued processing after individual path or traversal errors. Follow explicitly supplied symlinks to directories for enumeration.

### T7: Verify the acceptance contract

Add reusable-interface and executable-level tests for the behaviors and failure cases defined above, including operand grouping, symlink metadata and traversal, filename escaping, `--`, and continuation after mixed valid and invalid operands; then confirm the command builds and the test suite passes.
