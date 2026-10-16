# Plan: Implement T3 Option Parsing and Output Layout

## Summary

Add a CLI parsing boundary that converts `args_os` input into `ListingOptions` and path
operands, then connect it to the existing `list` interface. Support every short option in the
specification, combined short-option groups, and `--`. Keep option parsing separate from
filesystem listing so parse failures can be reported before any listing work begins. Print
entries on one line by default, with `-1` selecting one entry per line. Terminal-width-aware
column formatting remains out of scope.

## Implementation Changes

- Add a small CLI argument parser (in `src/main.rs` or a private `src/cli.rs`) that returns
  parsed listing options and `PathBuf` operands, or a parse error suitable for a diagnostic.
  Consume `std::env::args_os()` so operands retain non-UTF-8 filename bytes.
- Parse combined short options character by character. Map `a`, `A`, `l`, `h`, `R`, `r`, `t`,
  `S`, and `1` to the existing `ListingOptions` fields. Preserve the spec's sort precedence by
  selecting `Size` whenever `-S` is present, even if `-t` appears later; otherwise select
  `ModifiedTime` when `-t` is present.
- Recognize `--` as the end of option parsing. After it, treat every argument as an operand,
  including strings beginning with `-`. Before it, treat dash-prefixed arguments as options and
  report an unsupported option clearly, including its spelling, if any short option is unknown.
- Handle an otherwise unsupported dash-prefixed argument and a non-UTF-8 option argument as
  parse errors. Render the diagnostic to standard error and return a failure exit code without
  invoking `list`.
- Pass the successfully parsed options and operands to `list`. Keep filesystem diagnostics on
  standard error and preserve the existing rule that any listing error makes the process exit
  unsuccessfully while successful entries are still printed.
- Render entry names separated by spaces on one line by default. When `-1` is set, render one
  entry per line. Do not add terminal-width detection or multi-column layout.
- Update README option documentation to reflect that combined short options and `--` are now
  supported, and remove wording that presents those parser behaviors as merely planned.

## Tests

- Unit-test parsing of individual supported options and combined groups, including a mixed group
  such as `-al1` and the `-1` option by itself.
- Verify the `-S` precedence regardless of option order, and verify `-r`, `-a`/`-A`, and other
  flags populate the expected `ListingOptions` fields.
- Verify `--` leaves subsequent dash-prefixed strings as operands and that ordinary operands
  remain in their original order.
- Verify unsupported options produce a useful parse error, do not call listing, write a
  diagnostic to standard error, and return a failure status.
- Add executable-level coverage for default single-line output, `-1` output (one entry per line),
  combined options, `--` with a dash-prefixed filename, and unsupported-option exit status and
  diagnostic.
- Use OS-native argument construction in tests where needed so operand bytes are not forced
  through UTF-8 conversion.

## Boundaries and Assumptions

- T3 owns argument parsing and the `-1` CLI behavior; filtering, traversal, metadata formatting,
  and sorting implementation remain in their respective tasks.
- The reusable `list` API continues to accept parsed options and operands and does not parse
  argv, render CLI diagnostics, or determine process exit status.
- Parsing errors stop the command before listing; filesystem errors continue to follow the
  existing `ListingResult` aggregation and CLI status behavior.
- The first-version option set is exactly `-a`, `-A`, `-l`, `-h`, `-R`, `-r`, `-t`, `-S`, and
  `-1`, with no long options beyond the `--` terminator.
