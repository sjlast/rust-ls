//! Reusable listing types and behavior for the `rust-ls` command.

use std::ffi::OsString;
use std::fs::Metadata;
use std::io;
use std::path::PathBuf;

/// Options that control directory listing behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ListingOptions {
    /// Include `.` and `..` when listing directories.
    pub all: bool,
    /// Include hidden entries except `.` and `..`.
    pub almost_all: bool,
    /// Request long-format metadata in rendered output.
    pub long: bool,
    /// Render long-format sizes in readable units.
    pub human_readable: bool,
    /// Recurse into directories.
    pub recursive: bool,
    /// Reverse the selected ordering.
    pub reverse: bool,
    /// Select the ordering for entries.
    pub sort: SortOrder,
    /// Render one entry per line.
    pub one_per_line: bool,
}

/// The key used to order listing entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    /// Sort entries by name.
    #[default]
    Name,
    /// Sort entries by modification time.
    ModifiedTime,
    /// Sort entries by size.
    Size,
}

/// A filesystem entry selected for a listing.
#[derive(Debug)]
pub struct ListingEntry {
    /// Full path used to access the entry.
    pub path: PathBuf,
    /// Name to display, retained as OS-native bytes where supported.
    pub name: OsString,
    /// Metadata selected for the entry.
    pub metadata: Metadata,
}

/// An error encountered while processing an operand or one of its entries.
#[derive(Debug, thiserror::Error)]
#[error("{path}: {source}")]
pub struct ListingError {
    /// Path whose processing failed.
    pub path: PathBuf,
    /// Entry name, when the error occurred while processing a child entry.
    pub entry_name: Option<OsString>,
    /// Underlying filesystem error.
    #[source]
    pub source: io::Error,
}

impl ListingError {
    /// Create an error associated with an operand path.
    pub fn for_path(path: PathBuf, source: io::Error) -> Self {
        Self {
            path,
            entry_name: None,
            source,
        }
    }

    /// Create an error associated with an entry inside a listed path.
    pub fn for_entry(path: PathBuf, entry_name: OsString, source: io::Error) -> Self {
        Self {
            path,
            entry_name: Some(entry_name),
            source,
        }
    }
}

/// Results from a listing request, allowing successful entries and errors together.
#[derive(Debug, Default)]
pub struct ListingResult {
    /// Entries successfully selected for output.
    pub entries: Vec<ListingEntry>,
    /// Errors encountered while processing operands or entries.
    pub errors: Vec<ListingError>,
}

/// List entries for parsed options and operands.
///
/// The returned result keeps successful entries and individual errors separate so callers can
/// render diagnostics and determine their own process status. An empty operand slice represents
/// the default current-directory request.
///
/// # Arguments
///
/// * `options` - Parsed behavior options.
/// * `operands` - Paths requested by the caller.
///
/// # Returns
///
/// Selected entries and any per-path or per-entry errors.
///
/// # Errors
///
/// Filesystem errors are collected in `ListingResult::errors`; this function does not fail the
/// entire request because one path could not be processed.
pub fn list(_options: &ListingOptions, _operands: &[PathBuf]) -> ListingResult {
    // Enumeration and filtering are introduced in T2. Keeping this boundary callable now lets
    // the command and future behavior share one stable interface.
    ListingResult::default()
}

#[cfg(test)]
mod tests {
    use super::{ListingEntry, ListingError, ListingOptions, ListingResult, SortOrder};
    use std::ffi::OsString;
    use std::fs;
    use std::io;
    use std::path::PathBuf;

    #[test]
    fn options_default_to_name_sorting_without_flags() {
        let options = ListingOptions::default();

        assert_eq!(options.sort, SortOrder::Name);
        assert!(!options.all);
        assert!(!options.almost_all);
        assert!(!options.long);
        assert!(!options.human_readable);
        assert!(!options.recursive);
        assert!(!options.reverse);
        assert!(!options.one_per_line);
    }

    #[test]
    fn listing_result_can_hold_entries_and_errors_independently() {
        let current_executable = std::env::current_exe().expect("test executable path exists");
        let metadata = fs::metadata(&current_executable).expect("test executable metadata exists");
        let entry = ListingEntry {
            path: current_executable,
            name: OsString::from("test-binary"),
            metadata,
        };
        let error = ListingError::for_path(
            PathBuf::from("missing"),
            io::Error::new(io::ErrorKind::NotFound, "not found"),
        );
        let result = ListingResult {
            entries: vec![entry],
            errors: vec![error],
        };

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].path, PathBuf::from("missing"));
        assert!(result.errors[0].entry_name.is_none());
    }

    #[test]
    fn listing_error_can_identify_a_child_entry() {
        let error = ListingError::for_entry(
            PathBuf::from("directory"),
            "child".into(),
            io::Error::new(io::ErrorKind::PermissionDenied, "denied"),
        );

        assert_eq!(error.path, PathBuf::from("directory"));
        assert_eq!(
            error.entry_name.as_deref(),
            Some(std::ffi::OsStr::new("child"))
        );
    }
}
