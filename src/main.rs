use std::ffi::{OsStr, OsString};
use std::fmt::Write as _;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::process::ExitCode;

use rust_ls::{ListingOptions, ListingResult, SortOrder, list};

fn main() -> ExitCode {
    let arguments = std::env::args_os().skip(1);
    let (options, operands) = match parse_arguments(arguments) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("rust-ls: {error}");
            return ExitCode::FAILURE;
        }
    };

    let result = list(&options, &operands);
    render_result(&result, options.one_per_line);

    if result.errors.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn parse_arguments(
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<(ListingOptions, Vec<PathBuf>), String> {
    let mut options = ListingOptions::default();
    let mut operands = Vec::new();
    let mut parsing_options = true;
    let mut sort_by_time = false;
    let mut sort_by_size = false;

    for argument in arguments {
        if parsing_options && argument == OsStr::new("--") {
            parsing_options = false;
            continue;
        }

        let bytes = argument.as_os_str().as_bytes();
        if parsing_options && bytes.first() == Some(&b'-') && bytes.len() > 1 {
            for option in &bytes[1..] {
                match option {
                    b'a' => options.all = true,
                    b'A' => options.almost_all = true,
                    b'l' => options.long = true,
                    b'h' => options.human_readable = true,
                    b'R' => options.recursive = true,
                    b'r' => options.reverse = true,
                    b't' => sort_by_time = true,
                    b'S' => sort_by_size = true,
                    b'1' => options.one_per_line = true,
                    other => {
                        let option = char::from(*other);
                        return Err(format!("unsupported option '-{option}'"));
                    }
                }
            }
        } else {
            operands.push(PathBuf::from(argument));
        }
    }

    options.sort = if sort_by_size {
        SortOrder::Size
    } else if sort_by_time {
        SortOrder::ModifiedTime
    } else {
        SortOrder::Name
    };

    Ok((options, operands))
}

fn render_result(result: &ListingResult, one_per_line: bool) {
    if one_per_line {
        for entry in &result.entries {
            println!("{}", escape_name(entry.name.as_os_str()));
        }
    } else if !result.entries.is_empty() {
        let names = result
            .entries
            .iter()
            .map(|entry| escape_name(entry.name.as_os_str()))
            .collect::<Vec<_>>();
        println!("{}", names.join(" "));
    }

    for error in &result.errors {
        eprintln!("{error}");
    }
}

fn escape_name(name: &OsStr) -> String {
    let bytes = name.as_bytes();
    let mut escaped = String::with_capacity(bytes.len());
    let mut remaining = bytes;

    while !remaining.is_empty() {
        match std::str::from_utf8(remaining) {
            Ok(valid) => {
                append_escaped_text(&mut escaped, valid);
                break;
            }
            Err(error) => {
                let valid_length = error.valid_up_to();
                let valid = std::str::from_utf8(&remaining[..valid_length])
                    .expect("valid_up_to identifies a valid UTF-8 prefix");
                append_escaped_text(&mut escaped, valid);
                remaining = &remaining[valid_length..];

                let invalid_length = error.error_len().unwrap_or(remaining.len());
                for byte in &remaining[..invalid_length] {
                    write!(escaped, "\\x{byte:02X}")
                        .expect("writing escaped bytes to a String cannot fail");
                }
                remaining = &remaining[invalid_length..];
            }
        }
    }

    escaped
}

fn append_escaped_text(output: &mut String, text: &str) {
    for character in text.chars() {
        if character == '\\' {
            output.push_str("\\\\");
        } else if character.is_control() {
            let mut encoded = [0; 4];
            for byte in character.encode_utf8(&mut encoded).as_bytes() {
                write!(output, "\\x{byte:02X}")
                    .expect("writing escaped bytes to a String cannot fail");
            }
        } else {
            output.push(character);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{escape_name, parse_arguments};
    use rust_ls::{ListingOptions, SortOrder};
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[test]
    fn parses_combined_short_options_and_operands() {
        let args = ["-al1", "path with spaces"].map(OsString::from).into_iter();

        let (options, operands) = parse_arguments(args).expect("arguments should parse");

        assert_eq!(
            options,
            ListingOptions {
                all: true,
                long: true,
                one_per_line: true,
                ..ListingOptions::default()
            }
        );
        assert_eq!(operands, [PathBuf::from("path with spaces")]);
    }

    #[test]
    fn size_sort_takes_precedence_in_either_option_order() {
        for args in [["-tS".to_owned()], ["-St".to_owned()]] {
            let (options, _) = parse_arguments(args.into_iter().map(OsString::from))
                .expect("supported options should parse");
            assert_eq!(options.sort, SortOrder::Size);
        }
    }

    #[test]
    fn double_dash_makes_following_options_operands() {
        let args = ["--", "-dash", "-1"].map(OsString::from).into_iter();

        let (options, operands) = parse_arguments(args).expect("arguments should parse");

        assert_eq!(options, ListingOptions::default());
        assert_eq!(operands, [PathBuf::from("-dash"), PathBuf::from("-1")]);
    }

    #[test]
    fn rejects_unsupported_options() {
        let error = parse_arguments([OsString::from("-z")]).expect_err("-z is unsupported");
        assert_eq!(error, "unsupported option '-z'");
    }

    #[cfg(unix)]
    #[test]
    fn escapes_invalid_utf8_filename_bytes() {
        use std::os::unix::ffi::OsStringExt;

        let name = OsString::from_vec(b"valid-\xFF-name".to_vec());

        assert_eq!(escape_name(name.as_os_str()), "valid-\\xFF-name");
    }
}
