use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveDateTime, Timelike};
use std::ffi::{OsStr, OsString};
use std::fmt::Write as _;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use users::{Groups, Users, UsersCache};

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
    let render_errors = render_result(&result, &options);

    if result.errors.is_empty() && render_errors.is_empty() {
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

fn render_result(result: &ListingResult, options: &ListingOptions) -> Vec<String> {
    let mut errors = Vec::new();
    let cache = UsersCache::new();
    let now = Local::now();
    let groups = if result.groups.is_empty() && !result.entries.is_empty() {
        vec![(0, result.entries.len(), false)]
    } else {
        result
            .groups
            .iter()
            .map(|group| (group.start, group.end, group.is_directory))
            .collect()
    };
    for (start, end, is_directory) in groups {
        let entries = &result.entries[start..end];
        if options.long && is_directory {
            let blocks = entries
                .iter()
                .map(|entry| entry.metadata.blocks())
                .sum::<u64>();
            println!("total {blocks}");
        }
        if options.long {
            for entry in entries {
                let (line, entry_errors) =
                    render_long_entry(entry, options.human_readable, &cache, now);
                println!("{line}");
                errors.extend(entry_errors);
            }
        } else if options.one_per_line {
            for entry in entries {
                println!("{}", escape_name(entry.name.as_os_str()));
            }
        } else if !entries.is_empty() {
            let names = entries
                .iter()
                .map(|entry| escape_name(entry.name.as_os_str()))
                .collect::<Vec<_>>();
            println!("{}", names.join(" "));
        }
    }
    for error in &result.errors {
        eprintln!("{error}");
    }
    for error in &errors {
        eprintln!("rust-ls: {error}");
    }
    errors
}

fn render_long_entry(
    entry: &rust_ls::ListingEntry,
    human_readable: bool,
    cache: &UsersCache,
    now: DateTime<Local>,
) -> (String, Vec<String>) {
    let metadata = &entry.metadata;
    let mode = metadata.mode();
    let mut errors = Vec::new();
    let marker = match attribute_marker(&entry.path) {
        Ok(marker) => marker,
        Err(error) => {
            errors.push(format!("{}: {error}", entry.path.display()));
            None
        }
    };
    let mut permissions = format!("{}{}", file_type(mode), permission_bits(mode));
    if let Some(marker) = marker {
        permissions.push(marker);
    }
    let owner = cache
        .get_user_by_uid(metadata.uid())
        .map(|user| user.name().to_string_lossy().into_owned())
        .unwrap_or_else(|| metadata.uid().to_string());
    let group = cache
        .get_group_by_gid(metadata.gid())
        .map(|group| group.name().to_string_lossy().into_owned())
        .unwrap_or_else(|| metadata.gid().to_string());
    let file_kind = mode & libc::S_IFMT as u32;
    let size = if file_kind == libc::S_IFCHR as u32 || file_kind == libc::S_IFBLK as u32 {
        let (major, minor) = mac_device_numbers(metadata.rdev());
        format!("{major}, {minor}")
    } else if human_readable {
        human_size(metadata.len())
    } else {
        metadata.len().to_string()
    };
    let modified = metadata
        .modified()
        .ok()
        .map(DateTime::<Local>::from)
        .map(|date| format_timestamp(date.naive_local(), now.naive_local()))
        .unwrap_or_else(|| "??? ?? ??:??".to_owned());
    let mut name = escape_name(entry.name.as_os_str());
    if metadata.file_type().is_symlink() {
        match std::fs::read_link(&entry.path) {
            Ok(target) => {
                write!(name, " -> {}", escape_name(target.as_os_str()))
                    .expect("writing to String cannot fail");
            }
            Err(error) => errors.push(format!("{}: {error}", entry.path.display())),
        }
    }
    (
        format!(
            "{permissions} {} {owner} {group} {size} {modified} {name}",
            metadata.nlink()
        ),
        errors,
    )
}

fn file_type(mode: u32) -> char {
    match mode & libc::S_IFMT as u32 {
        value if value == libc::S_IFREG as u32 => '-',
        value if value == libc::S_IFDIR as u32 => 'd',
        value if value == libc::S_IFLNK as u32 => 'l',
        value if value == libc::S_IFCHR as u32 => 'c',
        value if value == libc::S_IFBLK as u32 => 'b',
        value if value == libc::S_IFIFO as u32 => 'p',
        value if value == libc::S_IFSOCK as u32 => 's',
        0o160000 => 'w',
        _ => '?',
    }
}

fn permission_bits(mode: u32) -> String {
    let mut result = String::with_capacity(9);
    for (read, write, execute, special, lower, upper) in [
        (0o400, 0o200, 0o100, 0o4000, 's', 'S'),
        (0o040, 0o020, 0o010, 0o2000, 's', 'S'),
        (0o004, 0o002, 0o001, 0o1000, 't', 'T'),
    ] {
        result.push(if mode & read != 0 { 'r' } else { '-' });
        result.push(if mode & write != 0 { 'w' } else { '-' });
        result.push(if mode & special != 0 {
            if mode & execute != 0 { lower } else { upper }
        } else if mode & execute != 0 {
            'x'
        } else {
            '-'
        });
    }
    result
}

fn human_size(size: u64) -> String {
    const UNITS: [&str; 6] = [
        "Byte", "Kilobyte", "Megabyte", "Gigabyte", "Terabyte", "Petabyte",
    ];
    let mut unit = 0usize;
    let mut divisor = 1u64;
    while size / divisor >= 1024 && unit < UNITS.len() - 1 {
        divisor *= 1024;
        unit += 1;
    }
    let mut rounded = size / divisor + u64::from(size % divisor >= divisor / 2 && unit > 0);
    while rounded >= 1024 && unit < UNITS.len() - 1 {
        rounded = (rounded + 512) / 1024;
        unit += 1;
    }
    if unit == UNITS.len() - 1 && rounded > 9999 {
        return ">9999 Petabyte".to_owned();
    }
    format!("{rounded} {}", UNITS[unit])
}

fn subtract_calendar_months(value: NaiveDateTime, months: i32) -> NaiveDateTime {
    let month_index = value.year() * 12 + value.month0() as i32 - months;
    let year = month_index.div_euclid(12);
    let month = month_index.rem_euclid(12) as u32 + 1;
    let day = value.day().min(days_in_month(year, month));
    NaiveDate::from_ymd_opt(year, month, day)
        .expect("valid clipped calendar date")
        .and_hms_nano_opt(
            value.hour(),
            value.minute(),
            value.second(),
            value.nanosecond(),
        )
        .expect("time components are valid")
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    };
    next.expect("supported calendar date")
        .pred_opt()
        .expect("date has predecessor")
        .day()
}

fn format_timestamp(value: NaiveDateTime, now: NaiveDateTime) -> String {
    let before = subtract_calendar_months(now, 6);
    let after = subtract_calendar_months(now, -6);
    let day_first = locale_day_first();
    let format = if value < before || value > after {
        if day_first { "%e %b %Y" } else { "%b %e %Y" }
    } else {
        if day_first {
            "%e %b %H:%M"
        } else {
            "%b %e %H:%M"
        }
    };
    locale_format(value, format).unwrap_or_else(|| {
        value
            .format(if format.ends_with("%Y") {
                "%b %e %Y"
            } else {
                "%b %e %H:%M"
            })
            .to_string()
    })
}

fn locale_day_first() -> bool {
    let locale = std::env::var("LC_ALL")
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| {
            std::env::var("LC_TIME")
                .ok()
                .filter(|value| !value.is_empty())
        })
        .or_else(|| std::env::var("LANG").ok().filter(|value| !value.is_empty()));
    if let Some(locale) = locale.and_then(|locale| std::ffi::CString::new(locale).ok()) {
        unsafe {
            libc::setlocale(libc::LC_TIME, locale.as_ptr());
        }
    }
    let date_format = unsafe { libc::nl_langinfo(libc::D_FMT) };
    if date_format.is_null() {
        return false;
    }
    let format = unsafe { std::ffi::CStr::from_ptr(date_format) }.to_bytes();
    let month = [b"%m".as_slice(), b"%b", b"%B"]
        .iter()
        .filter_map(|token| find_format_token(format, token))
        .min();
    let day = [b"%d".as_slice(), b"%e"]
        .iter()
        .filter_map(|token| find_format_token(format, token))
        .min();
    matches!((month, day), (Some(month), Some(day)) if day < month)
}

fn find_format_token(format: &[u8], token: &[u8]) -> Option<usize> {
    format
        .windows(token.len())
        .position(|window| window == token)
}

fn locale_format(value: NaiveDateTime, format: &str) -> Option<String> {
    use std::ffi::CString;
    let format = CString::new(format).ok()?;
    let mut tm = unsafe { std::mem::zeroed::<libc::tm>() };
    tm.tm_sec = value.second() as libc::c_int;
    tm.tm_min = value.minute() as libc::c_int;
    tm.tm_hour = value.hour() as libc::c_int;
    tm.tm_mday = value.day() as libc::c_int;
    tm.tm_mon = value.month0() as libc::c_int;
    tm.tm_year = value.year() - 1900;
    tm.tm_wday = value.weekday().num_days_from_sunday() as libc::c_int;
    tm.tm_yday = value.ordinal0() as libc::c_int;
    let locale = std::env::var("LC_ALL")
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| {
            std::env::var("LC_TIME")
                .ok()
                .filter(|value| !value.is_empty())
        })
        .or_else(|| std::env::var("LANG").ok().filter(|value| !value.is_empty()));
    if let Some(locale) = locale.and_then(|locale| CString::new(locale).ok()) {
        unsafe {
            libc::setlocale(libc::LC_TIME, locale.as_ptr());
        }
    }
    let mut buffer = [0i8; 128];
    let length = unsafe { libc::strftime(buffer.as_mut_ptr(), buffer.len(), format.as_ptr(), &tm) };
    (length > 0).then(|| {
        unsafe { std::ffi::CStr::from_ptr(buffer.as_ptr()) }
            .to_string_lossy()
            .into_owned()
    })
}

fn mac_device_numbers(device: u64) -> (u64, u64) {
    (((device >> 24) & 0xff), (device & 0x00ff_ffff))
}

#[cfg(target_os = "macos")]
fn attribute_marker(path: &Path) -> std::io::Result<Option<char>> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::from_raw_os_error(libc::EINVAL))?;
    let attrs_len =
        unsafe { libc::listxattr(path.as_ptr(), std::ptr::null_mut(), 0, libc::XATTR_NOFOLLOW) };
    if attrs_len < 0 {
        return Err(std::io::Error::last_os_error());
    }
    if attrs_len > 0 {
        return Ok(Some('@'));
    }
    let acl = unsafe { acl_get_file(path.as_ptr(), 0x00000100) };
    if acl.is_null() {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ENOENT) || error.raw_os_error() == Some(libc::ENOTSUP)
        {
            return Ok(None);
        }
        return Err(error);
    }
    let mut first_entry = std::ptr::null_mut();
    let entry_status = unsafe { acl_get_entry(acl, 0, &mut first_entry) };
    unsafe {
        acl_free(acl.cast());
    }
    if entry_status < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok((entry_status == 1).then_some('+'))
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn acl_get_file(path: *const libc::c_char, kind: libc::c_int) -> *mut libc::c_void;
    fn acl_get_entry(
        acl: *mut libc::c_void,
        entry_id: libc::c_int,
        entry: *mut *mut libc::c_void,
    ) -> libc::c_int;
    fn acl_free(value: *mut libc::c_void) -> libc::c_int;
}

#[cfg(not(target_os = "macos"))]
fn attribute_marker(_path: &Path) -> std::io::Result<Option<char>> {
    Ok(None)
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
    use super::{
        escape_name, file_type, format_timestamp, human_size, parse_arguments, permission_bits,
    };
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

    #[test]
    fn renders_file_types_and_special_permission_bits() {
        assert_eq!(file_type(libc::S_IFREG as u32), '-');
        assert_eq!(file_type(libc::S_IFDIR as u32), 'd');
        assert_eq!(file_type(libc::S_IFLNK as u32), 'l');
        assert_eq!(file_type(libc::S_IFCHR as u32), 'c');
        assert_eq!(file_type(libc::S_IFBLK as u32), 'b');
        assert_eq!(file_type(libc::S_IFIFO as u32), 'p');
        assert_eq!(file_type(libc::S_IFSOCK as u32), 's');
        assert_eq!(file_type(0o160000), 'w');
        assert_eq!(permission_bits(0o4750), "rwsr-x---");
        assert_eq!(permission_bits(0o2640), "rw-r-S---");
        assert_eq!(permission_bits(0o1700), "rwx-----T");
    }

    #[test]
    fn human_sizes_round_half_up_and_promote_after_rounding() {
        assert_eq!(human_size(1), "1 Byte");
        assert_eq!(human_size(1023), "1023 Byte");
        assert_eq!(human_size(1024), "1 Kilobyte");
        assert_eq!(human_size(1535), "1 Kilobyte");
        assert_eq!(human_size(1536), "2 Kilobyte");
        assert_eq!(human_size(1024 * 1024 - 513), "1023 Kilobyte");
        assert_eq!(human_size(1024 * 1024 - 512), "1 Megabyte");
        assert_eq!(human_size(1024 * 1024 - 511), "1 Megabyte");
        assert_eq!(human_size(1024 * 1024 - 1), "1 Megabyte");
        assert_eq!(human_size(u64::MAX), ">9999 Petabyte");
    }

    #[test]
    fn six_calendar_month_cutoffs_use_time_at_the_boundary() {
        use chrono::{NaiveDate, NaiveTime};
        let now = NaiveDate::from_ymd_opt(2024, 8, 31)
            .expect("date is valid")
            .and_time(NaiveTime::from_hms_opt(12, 0, 0).expect("time is valid"));
        let exact = NaiveDate::from_ymd_opt(2024, 2, 29)
            .expect("date is valid")
            .and_time(NaiveTime::from_hms_opt(12, 0, 0).expect("time is valid"));
        assert!(format_timestamp(exact, now).ends_with("12:00"));
        let outside = exact - chrono::Duration::nanoseconds(1);
        assert!(format_timestamp(outside, now).ends_with("2024"));
        let future_exact = NaiveDate::from_ymd_opt(2025, 2, 28)
            .expect("date is valid")
            .and_time(NaiveTime::from_hms_opt(12, 0, 0).expect("time is valid"));
        assert!(format_timestamp(future_exact, now).ends_with("12:00"));
        assert!(
            format_timestamp(future_exact + chrono::Duration::nanoseconds(1), now)
                .ends_with("2025")
        );
    }
}
