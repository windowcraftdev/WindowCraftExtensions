// Made by Nordan
mod audio;
mod data;
mod image;
mod remux;
mod target;
mod text;

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use target::{Family, Target};

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
enum Error {
    Io { target: String, source: io::Error },
    Invalid(String),
    Reason(text::Reason),
    Already(Target),
}
impl Error {
    fn io(target: impl Into<String>, source: io::Error) -> Self {
        if source.kind() == io::ErrorKind::InvalidData {
            return Self::Reason(text::Reason::InvalidInput);
        }
        Self::Io {
            target: target.into(),
            source,
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { target, source } => write!(f, "{target}: {source}"),
            Self::Invalid(s) => f.write_str(s),
            Self::Reason(r) => f.write_str(text::Text::for_tag("en").reason(*r)),
            Self::Already(t) => write!(f, "already {}", t.value().to_uppercase()),
        }
    }
}
impl std::error::Error for Error {}

#[derive(Debug, PartialEq, Eq)]
struct Args {
    json: bool,
    lang: String,
    to: Option<Target>,
    files: Vec<PathBuf>,
}
fn parse_args(args: impl IntoIterator<Item = OsString>) -> Args {
    let mut args = args.into_iter().peekable();
    let mut parsed = Args {
        json: false,
        lang: "en".into(),
        to: None,
        files: Vec::new(),
    };
    while let Some(arg) = args.next() {
        if arg == "--" {
            parsed.files.extend(args.map(PathBuf::from));
            break;
        } else if arg == "--json" {
            parsed.json = true;
        } else if arg == "--lang" || arg == "--to" {
            let value = if args
                .peek()
                .is_some_and(|v| !v.to_string_lossy().starts_with("--"))
            {
                args.next().unwrap().to_string_lossy().into_owned()
            } else {
                String::new()
            };
            if arg == "--lang" {
                parsed.lang = value;
            } else {
                parsed.to = Target::parse(&value);
            }
        } else {
            parsed.files.push(arg.into());
            parsed.files.extend(args.map(PathBuf::from));
            break;
        }
    }
    parsed
}
fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}
fn emit(out: &mut impl Write, json: bool, key: &str, message: &str) -> io::Result<()> {
    if json {
        serde_json::to_writer(&mut *out, &serde_json::json!({key: message}))?;
        writeln!(out)?;
    } else {
        writeln!(out, "{message}")?;
    }
    out.flush()
}
fn progress(out: &mut impl Write, json: bool, share: f32, step: &str) -> io::Result<()> {
    if json {
        serde_json::to_writer(
            &mut *out,
            &serde_json::json!({"progress": share.clamp(0.0, 1.0), "step": step}),
        )?;
        writeln!(out)?;
    } else {
        writeln!(out, "{step}")?;
    }
    out.flush()
}
fn convert_file(source: &Path, target: Target, progress: &mut dyn FnMut(f32)) -> Result<PathBuf> {
    let output = target::output_path(source, target);
    let temporary = tempfile::Builder::new()
        .prefix(".wc-convert-")
        .suffix(".tmp")
        .tempfile_in(output.parent().unwrap_or(Path::new(".")))
        .map_err(|e| Error::io(file_name(&output), e))?
        .into_temp_path();
    let converted = match target.family() {
        Family::Image => image::convert(source, &temporary, target)?,
        Family::Audio => audio::convert(source, &temporary, target, progress)?,
        Family::Video => remux::remux(source, &temporary, target, progress)?,
        Family::Data => data::convert(source, &temporary, target)?,
    };
    if !converted {
        return Err(Error::Reason(text::Reason::Unsupported));
    }
    // Refuse a destination created by another process while conversion ran.
    temporary
        .persist_noclobber(&output)
        .map_err(|e| Error::io(file_name(&output), e.error))?;
    progress(1.0);
    Ok(output)
}
fn run(args: impl IntoIterator<Item = OsString>, mut out: impl Write) -> i32 {
    let args = parse_args(args);
    let text = text::Text::for_tag(&args.lang);
    let Some(target) = args.to else {
        let _ = emit(&mut out, args.json, "error", text.bad_target);
        return 1;
    };
    if args.files.is_empty() {
        let _ = emit(&mut out, args.json, "error", text.no_files);
        return 1;
    }
    let (mut saved, mut failed) = (0, 0);
    let (mut first_saved, mut first_error) = (String::new(), String::new());
    for (index, source) in args.files.iter().enumerate() {
        let name = file_name(source);
        let step = format!("{} / {}: {name}", index + 1, args.files.len());
        let total = args.files.len() as f32;
        if progress(&mut out, args.json, index as f32 / total, &step).is_err() {
            return 1;
        }
        let mut broken = false;
        let result = convert_file(source, target, &mut |share| {
            if !broken {
                broken =
                    progress(&mut out, args.json, (index as f32 + share) / total, &step).is_err();
            }
        });
        if broken {
            return 1;
        }
        match result {
            Ok(output) => {
                saved += 1;
                if saved == 1 {
                    first_saved = file_name(&output);
                }
            }
            Err(error) => {
                failed += 1;
                if failed == 1 {
                    let reason = match error {
                        Error::Reason(r) => text.reason(r).to_owned(),
                        Error::Already(t) => text.already.replace("{}", &t.value().to_uppercase()),
                        other => other.to_string(),
                    };
                    first_error = format!("{name}: {reason}");
                }
            }
        }
    }
    if saved > 0 {
        let mut summary = text::sentence(text.saved, saved, &first_saved);
        if failed > 0 {
            summary.push(' ');
            summary.push_str(&text::sentence(text.failures, failed, &first_error));
        }
        i32::from(emit(&mut out, args.json, "summary", &summary).is_err())
    } else {
        let _ = emit(
            &mut out,
            args.json,
            "error",
            &text::sentence(text.failures, failed, &first_error),
        );
        1
    }
}
fn main() {
    std::process::exit(run(std::env::args_os().skip(1), io::stdout().lock()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }
    fn lines(bytes: &[u8]) -> Vec<Value> {
        std::str::from_utf8(bytes)
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect()
    }
    #[test]
    fn options_accept_any_order_and_stop_at_first_file() {
        for values in [
            &["--json", "--lang", "pl", "--to", "png", "--", "a.jpg"][..],
            &["--to", "png", "--lang", "pl", "--json", "a.jpg"][..],
            &["--lang", "pl", "--json", "--to", "png", "a.jpg"][..],
        ] {
            assert_eq!(
                parse_args(args(values)),
                Args {
                    json: true,
                    lang: "pl".into(),
                    to: Some(Target::Png),
                    files: vec!["a.jpg".into()]
                }
            );
        }
        assert_eq!(
            parse_args(args(&["--", "--json", "--to"])).files,
            vec![PathBuf::from("--json"), PathBuf::from("--to")]
        );
        assert!(!parse_args(args(&["a.jpg", "--json"])).json);
        assert!(parse_args(args(&["--to", "--json"])).json);
        assert_eq!(parse_args(args(&["--lang"])).lang, "");
    }
    #[test]
    fn missing_unknown_target_and_unknown_language() {
        for values in [
            &["--json"][..],
            &["--json", "--to", "bad"][..],
            &["--json", "--to"][..],
            &["--json", "--lang", "unknown"][..],
        ] {
            let mut output = Vec::new();
            assert_eq!(run(args(values), &mut output), 1);
            assert_eq!(
                lines(&output),
                vec![json!({"error":"Missing or unknown target format (--to)."})]
            );
        }
        let mut output = Vec::new();
        assert_eq!(run(args(&["--json", "--to", "png"]), &mut output), 1);
        assert_eq!(
            lines(&output),
            vec![json!({"error":"Choose at least one file."})]
        );
    }
    #[test]
    fn mixed_results_preserve_sources_and_avoid_collisions() {
        let d = tempfile::tempdir().unwrap();
        let source = d.path().join("photo.jpg");
        ::image::RgbImage::new(16, 12).save(&source).unwrap();
        let original = std::fs::read(&source).unwrap();
        let occupied = d.path().join("photo.png");
        std::fs::write(&occupied, b"untouched").unwrap();
        let bad = d.path().join("bad.bin");
        std::fs::write(&bad, b"bad").unwrap();
        let mut input = args(&["--json", "--lang", "pl", "--to", "png", "--"]);
        input.extend([bad.into_os_string(), source.clone().into_os_string()]);
        let mut output = Vec::new();
        assert_eq!(run(input, &mut output), 0);
        let rows = lines(&output);
        assert_eq!(
            rows.last().unwrap(),
            &json!({"summary":"Przekonwertowano: 1. Pierwszy: photo (2).png Niepowodzenia: 1. Pierwszy błąd: bad.bin: nieobsługiwany format wejściowy"})
        );
        let shares: Vec<f64> = rows.iter().filter_map(|r| r["progress"].as_f64()).collect();
        assert_eq!(shares[0], 0.0);
        assert_eq!(*shares.last().unwrap(), 1.0);
        assert!(shares.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(std::fs::read(&source).unwrap(), original);
        assert_eq!(std::fs::read(occupied).unwrap(), b"untouched");
        assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), 4);
    }
    #[test]
    fn same_format_and_damaged_file_leave_no_temporary_output() {
        let d = tempfile::tempdir().unwrap();
        let source = d.path().join("a.png");
        ::image::RgbImage::new(4, 4).save(&source).unwrap();
        assert!(matches!(
            convert_file(&source, Target::Png, &mut |_| {}),
            Err(Error::Already(Target::Png))
        ));
        let broken = d.path().join("bad.png");
        std::fs::write(&broken, b"\x89PNG\r\n\x1a\n").unwrap();
        assert!(convert_file(&broken, Target::Jpg, &mut |_| {}).is_err());
        assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), 2);
    }
    #[test]
    fn each_json_line_is_flushed_and_broken_output_stops_work() {
        #[derive(Default)]
        struct Sink {
            bytes: Vec<u8>,
            flushes: usize,
        }
        impl Write for Sink {
            fn write(&mut self, b: &[u8]) -> io::Result<usize> {
                self.bytes.write(b)
            }
            fn flush(&mut self) -> io::Result<()> {
                self.flushes += 1;
                Ok(())
            }
        }
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> io::Result<()> {
                Err(io::ErrorKind::BrokenPipe.into())
            }
        }
        let d = tempfile::tempdir().unwrap();
        let source = d.path().join("a.jpg");
        ::image::RgbImage::new(4, 4).save(&source).unwrap();
        let mut input = args(&["--json", "--to", "png", "--"]);
        input.push(source.into_os_string());
        assert_eq!(run(input.clone(), Broken), 1);
        assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), 1);
        let mut sink = Sink::default();
        assert_eq!(run(input, &mut sink), 0);
        assert_eq!(sink.flushes, lines(&sink.bytes).len());
    }
}
