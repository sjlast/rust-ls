# Plan: Implement T4 Long Format and Readable Sizes

## Summary

Implement long-format rendering for `-l` in the CLI, including file type and permissions,
link count, owner, group, size, modification time, and escaped name. Implement `-h` as a size
display change only when `-l` is enabled. Keep filesystem selection in the reusable `list`
interface and formatting in the CLI. Follow the behavior in `specs/man-ls.txt`; where this
requires behavior beyond the current `specs/PLAN.md` scope, this T4 plan records the required
extension.

## Implementation Changes

- Add a focused renderer for a `ListingEntry` in `src/main.rs` or a private rendering module.
  Pass the parsed `ListingOptions` (or the `long` and `human_readable` fields) into
  `render_result`; its current signature receives only `one_per_line`, so it cannot select
  long output yet. Select long rendering when `ListingOptions.long` is true and pass
  `ListingOptions.human_readable` to the size formatter. Long format always emits one entry per
  line, regardless of `-1`.
- Read Unix metadata through `std::os::unix::fs::MetadataExt`. Build the type-and-permission
  field from the entry's non-following metadata: file type character followed by the nine
  permission positions, including setuid, setgid, and sticky-bit forms. Use `-`, `d`, `l`, `c`,
  `b`, `p`, `s`, and `w` for regular files, directories, symlinks, character devices, block
  devices, FIFOs, sockets, and whiteouts, respectively; use `?` for any other type. Render each
  long entry in this field order, separated by one space: type-and-permissions, link count, owner, group,
  size, modification time, and escaped name. Do not add column padding. Include link count,
  owner and group names, byte size, formatted modification time, and the existing escaped name.
  Emit permission triplets in owner, group, other order, with read, write, execute positions in
  each triplet. Use `r`, `w`, and `x` for ordinary bits; render setuid and setgid as `s` when
  their corresponding execute bit is set and `S` otherwise, and sticky as `t` when other-execute
  is set and `T` otherwise.
  Append `@` to the permission field when the entry has extended attributes; otherwise append
  `+` when it has extended security information such as an ACL. Do not print ACL contents because
  `-e` is outside the supported option set. Query extended attributes and ACLs with macOS APIs;
  if inspection fails, emit a diagnostic, mark the CLI result as failed, and still render the
  entry without an attribute marker.
- Resolve user and group IDs to macOS account names, falling back to their numeric UID or GID
  when the ID is unknown or lookup fails. Use the `users` crate for passwd/group database
  lookups with the `users = "0.11"` dependency constraint recorded in `Cargo.toml`; avoid
  shelling out. Keep lookup behind a small private resolver that uses the crate's `Users`
  interface and one `UsersCache` per render request to avoid repeating lookups for common IDs.
  Lookup failures must not prevent rendering the entry; display the numeric UID or GID in that
  case.
- Format the modification time using the documented macOS-style month/day/time-or-year layout,
  honoring the active locale (`LC_ALL`, then `LC_TIME`, then `LANG`) for month spelling and
  month/day order. Show the
  locale's month/day plus hour/minute unless the timestamp is more than six calendar months
  before or after the captured current time, in which case show the locale's month/day and year.
  Compute the six-month cutoff with calendar-month arithmetic, clipping to the last valid day of
  the target month when needed. Capture the current time once per rendered result and pass it into
  the timestamp formatter so cutoff behavior is deterministic
  in tests. Add `chrono = { version = "0.4", features = ["clock"] }` to `Cargo.toml` and use
  `chrono`'s `Local` conversion for local time. Use locale-aware system date formatting for the
  rendered month/day order and names. The cutoff is strict: a timestamp exactly six calendar
  months before or after the captured current time uses hour/minute; only a timestamp farther
  away uses the year. If reading an entry's modification
  time fails, render `??? ?? ??:??` and continue
  rendering the entry; do not let a timestamp conversion failure suppress other fields or entries.
- For `-h -l`, use base-2 units with the suffixes `Byte`, `Kilobyte`, `Megabyte`, `Gigabyte`,
  `Terabyte`, and `Petabyte`. Render an integer followed by one space and the suffix exactly as
  listed (for example, `1 Byte` or `2 Kilobyte`); do not add decimals or pluralize suffixes.
  Values below 1024 remain integer `Byte` values. For larger values, round to the nearest whole
  unit value (half up), using integer arithmetic to avoid floating-point boundary errors. After
  rounding, promote a value that reaches 1024 to the next suffix and round again. Test both the
  powers-of-1024 unit transitions and the half-up thresholds that round to 1024 and trigger
  promotion.
  Keep the numeric portion to four or fewer digits for every suffix, as required by the manual.
  If a value exceeds the largest four-digit Petabyte display, render `>9999 Petabyte` rather than
  printing five or more digits or silently wrapping the value. Keep the spelled-out suffixes
  (`Byte`, `Kilobyte`, and so on), which are the suffixes named by `specs/man-ls.txt`.
  For `-l` without `-h`, print the byte count. When `-h` is present without `-l`, retain the
  current non-long output because `-h` has no visible effect in that mode.
- For character and block devices, replace the size field with the device's major and minor
  numbers, formatted as `major, minor` from the `rdev` value, matching macOS `ls -l`; decode
  `rdev` using the macOS `dev_t` encoding (through a macOS API or an equivalent implementation),
  not a device-number encoding borrowed from another Unix platform. Do not humanize this field
  with `-h`. For other entries, display the metadata byte size.
- Preserve `escape_name` for all displayed names so spaces, Unicode, control characters, and
  invalid UTF-8 bytes continue to follow the specified encoding. For symlinks, append ` -> `
  followed by the escaped link target returned by `read_link`; keep the symlink's own metadata
  for the other fields. If reading a link target fails during rendering, retain the entry name,
  emit a diagnostic, and make the CLI exit nonzero while continuing other entries. Have rendering
  return its per-entry diagnostics to `main`, which combines them with listing errors when choosing
  the final exit status.
- Before each directory's long-format entries, print `total N`, where `N` is the sum of the
  filesystem allocation blocks (`MetadataExt::blocks`) for the entries selected for that
  directory. Include synthesized `.` and `..` only when `-a` selected them. Preserve directory
  listing group boundaries in the reusable result so totals precede the correct group, including
  during recursive listing.
- Do not introduce color, terminal-width alignment, or options outside T4.
- Use the symlink metadata already selected by `list`; long-format size and type must describe
  the symlink itself for discovered symlink entries, not its target.

## Tests

- Add focused renderer tests for file-type characters and permission strings across regular
  files, directories, symlinks, FIFOs, sockets, character devices, block devices, and whiteouts,
  plus special permission bits. Create each type on the host where possible; for types the host
  cannot create, test the formatter against representative file-type and mode-bit inputs rather
  than trying to construct `std::fs::Metadata` values.
- Test `@` for extended attributes, `+` for ACL/security metadata without extended attributes,
  and `@` precedence when both are present. Test that metadata-inspection failures report an
  error but do not suppress the entry.
- Verify through renderer or CLI tests that all required long-format fields appear in the
  specified order and that names and symlink targets use the existing escaping behavior. Test
  metadata selection and symlink behavior through the reusable listing interface; it selects
  entries and metadata but does not render long-format fields.
- Verify byte sizes without `-h`, readable size units and rounding boundaries with `-h -l`,
  including exact output spelling, half-up cases immediately below/at/above a promotion
  threshold, and that `-h` alone does not change short output.
- Verify device-number display for character and block devices where the host permits creating
  them; otherwise test the macOS `dev_t` decoder with known encoded values and document the host
  limitation. Also verify the decoded values appear in the long-format size field.
- Verify symlink long output uses link metadata and link-text size rather than target metadata.
- Verify symlink long output appends ` -> ` and the escaped link target, and that link-target
  lookup failures produce a diagnostic, nonzero status, and continued rendering.
- Verify each directory group has a `total N` line equal to the sum of selected entries' block
  counts, including the `-a` synthesized entries only when selected.
- Exercise timestamps just inside, exactly at, and just outside both six-calendar-month cutoffs,
  including past and future timestamps, using a fixed injected current time. Assert that the exact
  cutoff uses hour/minute and timestamps farther away use the year; test month/day ordering and
  names under available locales. Set `TZ=UTC` in timezone-sensitive CLI tests and use a controlled
  locale rather than assuming the developer machine's environment.
- Define and test the fallback for an unavailable modification time by passing `None` to the
  timestamp formatter: render `??? ?? ??:??` while retaining the entry's other metadata and name.
- Verify numeric UID/GID fallback when an account ID is unknown or account lookup fails, without
  depending on the host's account database contents; use the `users` crate's mock `Users`
  implementation or an injected resolver returning no record.
- Add CLI-level coverage that `-l` forces one entry per line even without `-1`, and that
  combining `-l`, `-h`, and other supported flags preserves their behavior.
- Keep assertions focused on field contents and ordering, avoiding fragile assumptions about
  column padding unless alignment is explicitly adopted as part of the implementation.

## Boundaries and Assumptions

- T4 owns long-format metadata rendering and human-readable size display. Sorting, recursion,
  filtering, and operand grouping remain in their corresponding tasks.
- `ListingEntry.metadata` remains the source of displayed metadata; this task does not change
  filesystem enumeration or follow symlinks. The reusable result must retain the per-directory
  grouping needed to render each block total.
- `-h` affects only the size field in long output. Readable sizes use base 2, the macOS unit
  suffixes, nearest-whole-unit half-up rounding, promotion after rounding reaches 1024, and at
  most four displayed digits.
- Long-format timestamps use local time and the active locale, switching to a year display when
  they are more than six calendar months before or after the current time. The current time is
  captured once per rendered result and injected into the formatter for deterministic boundary
  tests. An exact six-month difference remains in hour/minute format; an unavailable modification
  time uses `??? ?? ??:??` and does not prevent rendering the entry.
- Human-readable sizes use the full suffix names from the manual and never exceed four digits in
  the numeric portion; a size beyond `9999 Petabyte` displays as `>9999 Petabyte`.
- The project targets macOS, but unit tests should avoid unnecessary assumptions about account
  database contents by accepting numeric owner/group fallbacks.
