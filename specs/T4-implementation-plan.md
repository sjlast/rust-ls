# Plan: Implement T4 Long Format and Readable Sizes

## Summary

Implement long-format rendering for `-l` in the CLI, including file type and permissions,
link count, owner, group, size, modification time, and escaped name. Implement `-h` as a size
display change only when `-l` is enabled. Keep filesystem selection in the reusable `list`
interface and formatting in the CLI. Follow the macOS metadata conventions and the contract in
`specs/PLAN.md`.

## Implementation Changes

- Add a focused renderer for a `ListingEntry` in `src/main.rs` or a private rendering module.
  Pass the parsed `ListingOptions` (or the `long` and `human_readable` fields) into
  `render_result`; its current signature receives only `one_per_line`, so it cannot select
  long output yet. Select long rendering when `ListingOptions.long` is true and pass
  `ListingOptions.human_readable` to the size formatter. Long format always emits one entry per
  line, regardless of `-1`.
- Read Unix metadata through `std::os::unix::fs::MetadataExt`. Build the type-and-permission
  field from the entry's non-following metadata: file type character followed by the nine
  permission positions, including setuid, setgid, and sticky-bit forms. Include link count,
  owner and group names, byte size, formatted modification time, and the existing escaped name.
- Resolve user and group IDs to macOS account names, falling back to their numeric UID or GID
  when the ID is unknown or lookup fails. Use a small, focused dependency or a safe system
  interface for account lookup; avoid shelling out. Prefer a safe crate that supports macOS
  passwd/group database lookups, and document the selected crate and version in `Cargo.toml`.
  Keep lookup behind small private functions so its API can be changed without affecting the
  renderer. Lookup failures must not prevent rendering the entry; display the numeric UID or
  GID in that case.
- Format the modification time using the documented macOS-style month/day/time-or-year layout:
  show month, day, and hour/minute unless the timestamp is more than 180 days before or after
  the captured current time, in which case show month, day, and year. Treat six months as a
  fixed 180-day duration, not calendar-month arithmetic. Capture the current time once per
  rendered result and pass it into the timestamp formatter so cutoff behavior is deterministic
  in tests. Use local time consistently with macOS `ls`, and use an appropriate date/time crate
  if the standard library cannot provide the required local-time conversion cleanly. The cutoff
  is strict: a timestamp exactly 180 days before or after the captured current time uses
  hour/minute; only a timestamp farther away uses the year. If reading an entry's modification
  time fails, render a documented placeholder (for example, `??? ?? ??:??`) and continue
  rendering the entry; do not let a timestamp conversion failure suppress other fields or entries.
- For `-h -l`, use base-2 units with the suffixes `Byte`, `Kilobyte`, `Megabyte`, `Gigabyte`,
  `Terabyte`, and `Petabyte`, keeping the displayed value to four or fewer digits. Round to the
  nearest whole unit value (half up). After rounding, promote a value that reaches 1024 to the
  next suffix and round again; test values immediately below, at, and above each unit boundary.
  The suffix list ends at Petabyte; for values whose rounded Petabyte count exceeds four digits,
  retain the Petabyte suffix and print the full rounded count rather than overflow, truncate, or
  wrap it. This is the sole case where the four-digit limit does not apply.
  For `-l` without `-h`, print the byte count. When `-h` is present without `-l`, retain the
  current non-long output because `-h` has no visible effect in that mode.
- For character and block devices, replace the size field with the device's major and minor
  numbers, formatted as `major, minor` from the `rdev` value, matching macOS `ls -l`; do not
  humanize this field with `-h`. For other entries, display the metadata byte size.
- Preserve `escape_name` for all displayed names so spaces, Unicode, control characters, and
  invalid UTF-8 bytes continue to follow the specified encoding. Do not introduce color,
  terminal-width alignment, block counts, or other macOS options outside T4.
- Use the symlink metadata already selected by `list`; long-format size and type must describe
  the symlink itself for discovered symlink entries, not its target.

## Tests

- Add focused tests for file-type characters and permission strings across regular files,
  directories, symlinks, FIFOs, sockets, character devices, and block devices, plus special
  permission bits. Create each type on the host where possible; for types the host cannot create,
  test the formatter against representative type/mode metadata values.
- Verify all required long-format fields are rendered and that the name uses the existing
  escaping behavior.
- Verify byte sizes without `-h`, readable size units and rounding boundaries with `-h -l`,
  and that `-h` alone does not change short output.
- Verify device-number display for character and block devices where the host permits creating
  them; otherwise exercise the formatter with representative device metadata or document the
  host limitation.
- Verify symlink long output uses link metadata and link-text size rather than target metadata.
- Exercise timestamps just inside, exactly at, and just outside both 180-day cutoffs, including
  past and future timestamps, using a fixed injected current time. Assert that the exact cutoff
  uses hour/minute and timestamps farther away use the year; also verify local-time formatting.
- Define and test the fallback for an unavailable modification time: render the chosen timestamp
  placeholder while retaining the entry's other metadata and name.
- Verify numeric UID/GID fallback when an account ID is unknown or account lookup fails, without
  depending on the host's account database contents.
- Add CLI-level coverage that `-l` forces one entry per line even without `-1`, and that
  combining `-l`, `-h`, and other supported flags preserves their behavior.
- Keep assertions focused on field contents and ordering, avoiding fragile assumptions about
  column padding unless alignment is explicitly adopted as part of the implementation.

## Boundaries and Assumptions

- T4 owns long-format metadata rendering and human-readable size display. Sorting, recursion,
  filtering, and operand grouping remain in their corresponding tasks.
- `ListingEntry.metadata` remains the source of displayed metadata; this task does not change
  filesystem enumeration or follow symlinks.
- `-h` affects only the size field in long output. Readable sizes use base 2, the macOS unit
  suffixes, nearest-whole-unit half-up rounding, promotion after rounding reaches 1024, and at
  most four displayed digits.
- Long-format timestamps use local time and switch to a year display when they are more than
  180 days before or after the current time. The current time is captured once per rendered
  result and injected into the formatter for deterministic boundary tests. An exact 180-day
  difference remains in hour/minute format; an unavailable modification time uses the documented
  placeholder and does not prevent rendering the entry.
- Human-readable sizes promote through Petabyte. Values above four digits at that final suffix
  retain their full rounded count, so the width limit applies only while a larger suffix is
  available.
- The project targets macOS, but unit tests should avoid unnecessary assumptions about account
  database contents by accepting numeric owner/group fallbacks.
