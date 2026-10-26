use std::ffi::OsStr;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
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

#[test]
fn long_format_prints_metadata_one_entry_per_line_and_directory_total() {
    let directory = TestDirectory::new();
    fs::write(directory.path().join("a file"), "abc").expect("file can be created");
    fs::write(directory.path().join("second"), "").expect("file can be created");

    let output = run(directory.path(), &[OsStr::new("-lh")]);
    let stdout = String::from_utf8(output.stdout).expect("output is UTF-8");
    let lines = stdout.lines().collect::<Vec<_>>();
    let expected_blocks = ["a file", "second"]
        .iter()
        .map(|name| {
            fs::symlink_metadata(directory.path().join(name))
                .expect("metadata exists")
                .blocks()
        })
        .sum::<u64>();
    assert!(output.status.success());
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], format!("total {expected_blocks}"));
    assert!(lines[1].contains(" 3B "));
    assert!(lines[1].ends_with(" a file"));
    assert!(lines[2].ends_with(" second"));
    assert!(lines[1].starts_with("-"));
}

#[cfg(unix)]
#[test]
fn all_long_format_total_includes_synthesized_dot_entries_and_hidden_files() {
    let directory = TestDirectory::new();
    fs::write(directory.path().join("visible"), "data").expect("file can be created");
    fs::write(directory.path().join(".hidden"), "data").expect("hidden file can be created");

    let output = run(directory.path(), &[OsStr::new("-la")]);
    let stdout = String::from_utf8(output.stdout).expect("output is UTF-8");
    let lines = stdout.lines().collect::<Vec<_>>();
    let expected_blocks = [
        directory.path().to_path_buf(),
        directory.path().join(".."),
        directory.path().join("visible"),
        directory.path().join(".hidden"),
    ]
    .iter()
    .map(|path| {
        fs::symlink_metadata(path)
            .expect("selected metadata exists")
            .blocks()
    })
    .sum::<u64>();

    assert!(output.status.success());
    assert_eq!(lines[0], format!("total {expected_blocks}"));
    assert!(lines.iter().any(|line| line.ends_with(" .")));
    assert!(lines.iter().any(|line| line.ends_with(" ..")));
    assert!(lines.iter().any(|line| line.ends_with(" .hidden")));
}

#[cfg(unix)]
#[test]
fn long_format_shows_symlink_size_and_escaped_target() {
    use std::os::unix::fs::symlink;
    let directory = TestDirectory::new();
    fs::write(
        directory.path().join("target with space"),
        "a much longer target",
    )
    .expect("file can be created");
    symlink("target with space", directory.path().join("link name")).expect("link can be created");

    let output = run(
        directory.path(),
        &[OsStr::new("-l"), OsStr::new("link name")],
    );
    let stdout = String::from_utf8(output.stdout).expect("output is UTF-8");
    assert!(output.status.success());
    assert!(stdout.starts_with("l"));
    assert!(stdout.contains(" 17 "));
    assert!(stdout.ends_with("link name -> target with space\n"));
}
