use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

struct TestDirectory(PathBuf);

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

impl TestDirectory {
    fn new() -> Self {
        loop {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("rust-ls-cli-{}-{id}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("test directory can be created: {error}"),
            }
        }
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(directory: &Path, arguments: &[&OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rust-ls"))
        .current_dir(directory)
        .args(arguments)
        .output()
        .expect("binary should run")
}

#[test]
fn default_output_joins_names_on_one_line_and_escapes_special_bytes() {
    let directory = TestDirectory::new();
    fs::write(directory.path().join("a space"), "").expect("file can be created");
    fs::write(directory.path().join("line\nbreak"), "").expect("file can be created");
    fs::write(directory.path().join("slash\\name"), "").expect("file can be created");
    let output = run(directory.path(), &[]);

    assert!(output.status.success());
    assert_eq!(output.stdout, b"a space line\\x0Abreak slash\\\\name\n");
}

#[test]
fn one_option_prints_each_name_on_a_separate_line() {
    let directory = TestDirectory::new();
    fs::write(directory.path().join("beta"), "").expect("file can be created");
    fs::write(directory.path().join("alpha"), "").expect("file can be created");

    let output = run(directory.path(), &[OsStr::new("-1")]);

    assert!(output.status.success());
    assert_eq!(output.stdout, b"alpha\nbeta\n");
}

#[test]
fn double_dash_allows_listing_a_dash_prefixed_filename() {
    let directory = TestDirectory::new();
    fs::write(directory.path().join("-dash"), "").expect("file can be created");

    let output = run(directory.path(), &[OsStr::new("--"), OsStr::new("-dash")]);

    assert!(output.status.success());
    assert_eq!(output.stdout, b"-dash\n");
}

#[test]
fn unsupported_option_fails_with_diagnostic_without_listing() {
    let directory = TestDirectory::new();

    let output = run(directory.path(), &[OsStr::new("-z")]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"rust-ls: unsupported option '-z'\n");
}
