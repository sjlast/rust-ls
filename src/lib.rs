//! Reusable listing types and behavior for the `rust-ls` command.

use std::ffi::{OsStr, OsString};
use std::fs::{self, Metadata};
use std::io;
use std::path::{Path, PathBuf};

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
pub fn list(options: &ListingOptions, operands: &[PathBuf]) -> ListingResult {
    let default_operand = PathBuf::from(".");
    let requested_operands = if operands.is_empty() {
        std::slice::from_ref(&default_operand)
    } else {
        operands
    };

    let mut result = ListingResult::default();
    let mut file_entries = Vec::new();
    let mut directory_operands = Vec::new();

    for operand in requested_operands {
        match fs::metadata(operand) {
            Ok(metadata) if metadata.is_dir() => directory_operands.push(operand),
            Ok(_) => match fs::symlink_metadata(operand) {
                Ok(metadata) => file_entries.push(ListingEntry {
                    path: operand.clone(),
                    name: operand
                        .file_name()
                        .map(OsStr::to_os_string)
                        .unwrap_or_else(|| operand.as_os_str().to_os_string()),
                    metadata,
                }),
                Err(source) => result
                    .errors
                    .push(ListingError::for_path(operand.clone(), source)),
            },
            Err(source) => result
                .errors
                .push(ListingError::for_path(operand.clone(), source)),
        }
    }

    file_entries.sort_by(|left, right| left.name.cmp(&right.name));
    result.entries = file_entries;

    for directory in directory_operands {
        list_directory(directory, options, &mut result);
    }

    result
}

fn list_directory(directory: &Path, options: &ListingOptions, result: &mut ListingResult) {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(source) => {
            result
                .errors
                .push(ListingError::for_path(directory.to_path_buf(), source));
            return;
        }
    };

    let mut directory_entries = Vec::new();
    for entry_result in entries {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(source) => {
                result
                    .errors
                    .push(ListingError::for_path(directory.to_path_buf(), source));
                continue;
            }
        };

        let name = entry.file_name();
        if should_include(&name, options) {
            let path = directory.join(&name);
            match fs::symlink_metadata(&path) {
                Ok(metadata) => directory_entries.push(ListingEntry {
                    path,
                    name,
                    metadata,
                }),
                Err(source) => result.errors.push(ListingError::for_entry(
                    directory.to_path_buf(),
                    name,
                    source,
                )),
            }
        }
    }

    if options.all {
        for name in [OsString::from("."), OsString::from("..")] {
            let path = directory.join(&name);
            match fs::symlink_metadata(&path) {
                Ok(metadata) => directory_entries.push(ListingEntry {
                    path,
                    name,
                    metadata,
                }),
                Err(source) => result.errors.push(ListingError::for_entry(
                    directory.to_path_buf(),
                    name,
                    source,
                )),
            }
        }
    }

    directory_entries.sort_by(|left, right| left.name.cmp(&right.name));
    result.entries.extend(directory_entries);
}

fn should_include(name: &OsStr, options: &ListingOptions) -> bool {
    let name_text = name.to_string_lossy();
    if name_text == "." || name_text == ".." {
        return options.all;
    }

    !name_text.starts_with('.') || options.all || options.almost_all
}

#[cfg(test)]
mod tests {
    use super::{ListingEntry, ListingError, ListingOptions, ListingResult, SortOrder};
    use std::ffi::OsString;
    use std::fs;
    use std::io;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct TestDirectory(PathBuf);

    static NEXT_TEST_DIRECTORY_ID: AtomicU64 = AtomicU64::new(0);

    impl TestDirectory {
        fn new() -> Self {
            loop {
                let id = NEXT_TEST_DIRECTORY_ID.fetch_add(1, Ordering::Relaxed);
                let path =
                    std::env::temp_dir().join(format!("rust-ls-test-{}-{id}", std::process::id()));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("test directory can be created: {error}"),
                }
            }
        }

        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn entry_names(result: &ListingResult) -> Vec<String> {
        result
            .entries
            .iter()
            .map(|entry| entry.name.to_string_lossy().into_owned())
            .collect()
    }

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

    #[test]
    fn lists_directory_entries_in_name_order_and_hides_dotfiles_by_default() {
        let directory = TestDirectory::new();
        fs::write(directory.path().join("zeta"), "").expect("file can be created");
        fs::write(directory.path().join(".hidden"), "").expect("hidden file can be created");
        fs::write(directory.path().join("alpha"), "").expect("file can be created");

        let result = super::list(
            &ListingOptions::default(),
            &[directory.path().to_path_buf()],
        );

        assert!(result.errors.is_empty());
        assert_eq!(entry_names(&result), ["alpha", "zeta"]);
    }

    #[test]
    fn almost_all_includes_hidden_entries_but_all_also_synthesizes_dot_entries() {
        let directory = TestDirectory::new();
        fs::write(directory.path().join(".hidden"), "").expect("hidden file can be created");
        let operand = [directory.path().to_path_buf()];

        let almost_all = super::list(
            &ListingOptions {
                almost_all: true,
                ..ListingOptions::default()
            },
            &operand,
        );
        assert_eq!(entry_names(&almost_all), [".hidden"]);

        let all = super::list(
            &ListingOptions {
                all: true,
                ..ListingOptions::default()
            },
            &operand,
        );
        assert_eq!(entry_names(&all), [".", "..", ".hidden"]);
    }

    #[test]
    fn all_takes_precedence_when_both_hidden_options_are_set() {
        let directory = TestDirectory::new();
        let result = super::list(
            &ListingOptions {
                all: true,
                almost_all: true,
                ..ListingOptions::default()
            },
            &[directory.path().to_path_buf()],
        );

        assert_eq!(entry_names(&result), [".", ".."]);
    }

    #[test]
    fn sorts_file_operands_as_a_group_before_directories_in_operand_order() {
        let directory = TestDirectory::new();
        let first_directory = directory.path().join("first");
        let second_directory = directory.path().join("second");
        fs::create_dir(&first_directory).expect("first directory can be created");
        fs::create_dir(&second_directory).expect("second directory can be created");
        fs::write(first_directory.join("inside-first"), "").expect("file can be created");
        fs::write(second_directory.join("inside-second"), "").expect("file can be created");
        let z_file = directory.path().join("z-file");
        let a_file = directory.path().join("a-file");
        fs::write(&z_file, "").expect("file can be created");
        fs::write(&a_file, "").expect("file can be created");

        let result = super::list(
            &ListingOptions::default(),
            &[second_directory, z_file, first_directory, a_file],
        );

        assert!(result.errors.is_empty());
        assert_eq!(
            entry_names(&result),
            ["a-file", "z-file", "inside-second", "inside-first"]
        );
    }

    #[cfg(unix)]
    #[test]
    fn discovered_symlink_uses_link_metadata_and_explicit_directory_symlink_is_followed() {
        use std::os::unix::fs::symlink;

        let directory = TestDirectory::new();
        let target_directory = directory.path().join("target");
        fs::create_dir(&target_directory).expect("target directory can be created");
        fs::write(target_directory.join("child"), "data").expect("file can be created");
        let directory_link = directory.path().join("directory-link");
        symlink(&target_directory, &directory_link).expect("directory symlink can be created");
        let file_target = directory.path().join("target-file");
        fs::write(&file_target, "a longer target").expect("file can be created");
        let file_link = directory.path().join("file-link");
        symlink("target-file", &file_link).expect("file symlink can be created");

        let discovered = super::list(
            &ListingOptions::default(),
            &[directory.path().to_path_buf()],
        );
        let link_entry = discovered
            .entries
            .iter()
            .find(|entry| entry.name == "file-link")
            .expect("file symlink is listed");
        assert!(link_entry.metadata.file_type().is_symlink());
        assert_eq!(link_entry.metadata.len(), "target-file".len() as u64);

        let followed = super::list(&ListingOptions::default(), &[directory_link]);
        assert_eq!(entry_names(&followed), ["child"]);
    }

    #[test]
    fn missing_operands_report_errors_while_valid_operands_still_list() {
        let directory = TestDirectory::new();
        let valid_file = directory.path().join("valid");
        fs::write(&valid_file, "").expect("file can be created");
        let missing_file = directory.path().join("missing");

        let result = super::list(
            &ListingOptions::default(),
            &[missing_file.clone(), valid_file],
        );

        assert_eq!(entry_names(&result), ["valid"]);
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].path, missing_file);
    }

    #[test]
    fn empty_operands_list_the_current_directory() {
        let current_directory = std::env::current_dir().expect("current directory is available");
        let result = super::list(&ListingOptions::default(), &[]);
        let expected = fs::read_dir(current_directory)
            .expect("current directory is readable")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name())
            .filter(|name| !name.to_string_lossy().starts_with('.'))
            .collect::<Vec<_>>();
        let mut expected = expected;
        expected.sort();

        assert_eq!(
            result
                .entries
                .iter()
                .map(|entry| &entry.name)
                .collect::<Vec<_>>(),
            expected.iter().collect::<Vec<_>>()
        );
        assert!(result.errors.is_empty());
    }
}
