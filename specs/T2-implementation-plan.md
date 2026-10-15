# Plan: Implement T2 Operand Handling, Filtering, and Name Sorting

## Summary

Implement the listing behavior in `list` using the existing `ListingOptions`, `ListingEntry`, and `ListingResult` types. Use the T2 rules in the spec and treat `-a` as taking precedence over `-A` when both are set.

## Implementation Changes

- With no operands, list the current directory. Display explicit file operands as entries; enumerate explicit directory operands. Follow an explicitly supplied symlink to a directory for enumeration.
- Filter discovered directory entries by name: omit dot-prefixed entries by default; include hidden entries but omit `.` and `..` with `-A`; include all entries with `-a`, synthesizing `.` and `..` with metadata from the listed directory and its parent.
- Sort file operands together by ascending, case-sensitive name. Keep directory operands in argument order and sort each directory’s contents separately. Use non-following metadata for discovered entries.
- Collect path and entry errors in `ListingResult` while continuing with other operands. Keep the function independent of CLI parsing, diagnostics, and exit status.

## Tests

- Exercise default current-directory behavior, explicit files, directories, and symlink-to-directory operands.
- Cover hidden filtering, `-A`, `-a` synthesis, and the chosen `-a` precedence when both flags are set.
- Verify file grouping and name order, directory operand order, per-directory ordering, and non-following metadata for discovered symlinks.
- Verify missing or inaccessible paths produce collected errors without discarding successful entries.

## Assumptions

- Dotfile filtering applies to entries discovered during directory enumeration; an explicitly named file operand is still displayed.
- Existing listing types remain the public interface; no new dependencies or CLI parsing changes are needed for T2.
