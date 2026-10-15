use std::path::PathBuf;
use std::process::ExitCode;

use rust_ls::{ListingOptions, ListingResult, list};

fn main() -> ExitCode {
    let operands = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    let result = list(&ListingOptions::default(), &operands);
    render_result(&result);

    if result.errors.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn render_result(result: &ListingResult) {
    for entry in &result.entries {
        println!("{}", entry.name.to_string_lossy());
    }
    for error in &result.errors {
        eprintln!("{error}");
    }
}
