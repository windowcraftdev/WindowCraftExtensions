// Made by Nordan
mod containers;
mod strip;
mod text;

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use text::Text;

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
enum Error {
    Io { target: String, source: io::Error },
    Unsupported(String),
    Invalid(String),
}

impl Error {
    fn io(target: impl Into<String>, source: io::Error) -> Self {
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
            Self::Unsupported(detail) => write!(f, "unsupported format: {detail}"),
            Self::Invalid(detail) => write!(f, "invalid value: {detail}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Args {
    json: bool,
    lang: String,
    files: Vec<PathBuf>,
}

// Arguments exclude the executable name. The first file ends option parsing.
fn parse_args(args: impl IntoIterator<Item = OsString>) -> Args {
    let mut args = args.into_iter().peekable();
    let mut parsed = Args {
        json: false,
        lang: "en".into(),
        files: Vec::new(),
    };
    while let Some(arg) = args.next() {
        if arg == "--" {
            parsed.files.extend(args.map(PathBuf::from));
            break;
        } else if arg == "--json" {
            parsed.json = true;
        } else if arg == "--lang" {
            if args
                .peek()
                .is_some_and(|tag| !tag.to_string_lossy().starts_with("--"))
            {
                parsed.lang = args.next().unwrap().to_string_lossy().into_owned();
            } else {
                parsed.lang = "en".into();
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

fn emit(stdout: &mut impl Write, json: bool, key: &str, message: &str) -> io::Result<()> {
    if json {
        serde_json::to_writer(&mut *stdout, &serde_json::json!({key: message}))?;
        writeln!(stdout)?;
        stdout.flush()
    } else if key == "error" {
        let mut stderr = io::stderr().lock();
        writeln!(stderr, "{message}")?;
        stderr.flush()
    } else {
        writeln!(stdout, "{message}")?;
        stdout.flush()
    }
}

pub fn run(args: impl IntoIterator<Item = OsString>, mut stdout: impl Write) -> i32 {
    let args = parse_args(args);
    let text = Text::for_tag(&args.lang);
    let mut saved = 0;
    let mut failed = 0;
    let mut first_saved = String::new();
    let mut first_error = String::new();
    for (index, source) in args.files.iter().enumerate() {
        let name = file_name(source);
        let step = format!("{} / {}: {name}", index + 1, args.files.len());
        let progress = if args.json {
            serde_json::to_writer(
                &mut stdout,
                &serde_json::json!({
                    "progress": index as f64 / args.files.len() as f64, "step": step
                }),
            )
            .map_err(io::Error::from)
            .and_then(|()| writeln!(stdout))
            .and_then(|()| stdout.flush())
        } else {
            writeln!(stdout, "{step}").and_then(|()| stdout.flush())
        };
        if progress.is_err() {
            return 1;
        }
        match strip::strip_file(source) {
            Ok(result) => {
                saved += 1;
                if saved == 1 {
                    first_saved = file_name(&result.output);
                }
            }
            Err(error) => {
                failed += 1;
                if failed == 1 {
                    let reason = match error {
                        Error::Unsupported(_) => text.unsupported.to_owned(),
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
        i32::from(emit(&mut stdout, args.json, "summary", &summary).is_err())
    } else {
        let error = if args.files.is_empty() {
            text.no_files
        } else {
            &first_error
        };
        let _ = emit(&mut stdout, args.json, "error", error);
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

    fn args(items: &[&str]) -> Vec<OsString> {
        items.iter().map(OsString::from).collect()
    }

    fn lines(output: &[u8]) -> Vec<Value> {
        std::str::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn options_accept_either_order_and_stop_at_files_or_separator() {
        for items in [
            args(&["--json", "--lang", "pl", "a.jpg", "b.mp3"]),
            args(&["--lang", "pl", "--json", "a.jpg", "b.mp3"]),
        ] {
            assert_eq!(
                parse_args(items),
                Args {
                    json: true,
                    lang: "pl".into(),
                    files: vec!["a.jpg".into(), "b.mp3".into()]
                }
            );
        }
        assert_eq!(
            parse_args(args(&["--", "--json", "--lang"])).files,
            vec![PathBuf::from("--json"), PathBuf::from("--lang")]
        );
        assert!(!parse_args(args(&["--", "--json"])).json);
        assert_eq!(
            parse_args(args(&["a.jpg", "--json"])).files,
            vec![PathBuf::from("a.jpg"), PathBuf::from("--json")]
        );
        assert_eq!(parse_args(args(&["--lang"])).lang, "en");
        assert!(parse_args(args(&["--lang", "--json"])).json);
    }

    #[test]
    fn no_files_and_unknown_language_return_english_json_error() {
        for items in [
            args(&["--json"]),
            args(&["--json", "--lang"]),
            args(&["--json", "--lang", "unknown"]),
        ] {
            let mut output = Vec::new();
            assert_eq!(run(items, &mut output), 1);
            assert_eq!(
                lines(&output),
                vec![json!({"error": "Choose at least one file."})]
            );
        }
    }

    fn atom(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        [
            ((payload.len() + 8) as u32).to_be_bytes().as_slice(),
            kind,
            payload,
        ]
        .concat()
    }

    fn jpeg_with_gps() -> Vec<u8> {
        let mut exif = b"Exif\0\0II\x2A\0\x08\0\0\0".to_vec();
        exif.extend_from_slice(&2u16.to_le_bytes());
        // IFD0 points to a GPS IFD after the orientation and next-IFD fields.
        exif.extend_from_slice(&[0x25, 0x88, 4, 0, 1, 0, 0, 0, 38, 0, 0, 0]);
        exif.extend_from_slice(&[0x12, 1, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0]);
        exif.extend_from_slice(&[0; 4]);
        exif.extend_from_slice(&1u16.to_le_bytes());
        exif.extend_from_slice(&[1, 0, 2, 0, 2, 0, 0, 0, b'N', 0, 0, 0]);
        exif.extend_from_slice(&[0; 4]);
        let mut data = vec![0xFF, 0xD8, 0xFF, 0xE1];
        data.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        data.extend_from_slice(&exif);
        data.extend_from_slice(&[0xFF, 0xDA, 0, 4, 1, 0, 0x12, 0xFF, 0, 0x34, 0xFF, 0xD9]);
        data
    }

    fn check_clean(name: &str, original: &[u8], check: impl FnOnce(&[u8])) {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join(name);
        std::fs::write(&source, original).unwrap();
        let saved = strip::clean_path(&source);
        let mut output = Vec::new();
        assert_eq!(
            run(
                vec![
                    "--json".into(),
                    "--lang".into(),
                    "pl".into(),
                    source.clone().into_os_string()
                ],
                &mut output
            ),
            0
        );
        assert_eq!(
            lines(&output),
            vec![
                json!({"progress": 0.0, "step": format!("1 / 1: {name}")}),
                json!({"summary": format!("Zapisano czyste kopie: 1. Pierwsza: {}", file_name(&saved))}),
            ]
        );
        assert_eq!(std::fs::read(source).unwrap(), original);
        check(&std::fs::read(saved).unwrap());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn jpeg_json_run_removes_exif_gps_and_keeps_orientation() {
        let original = jpeg_with_gps();
        check_clean("photo.jpg", &original, |clean| {
            assert!(!clean.windows(2).any(|w| w == [0x25, 0x88]));
            assert!(
                clean
                    .windows(12)
                    .any(|w| w == [1, 0x12, 0, 3, 0, 0, 0, 1, 0, 6, 0, 0])
            );
            assert!(clean.ends_with(&[0x12, 0xFF, 0, 0x34, 0xFF, 0xD9]));
        });
    }

    #[test]
    fn mp3_json_run_removes_id3_and_keeps_audio() {
        check_clean(
            "song.mp3",
            b"ID3\x04\0\0\0\0\0\x05TIT2x\xFF\xFBaudio",
            |clean| assert_eq!(clean, b"\xFF\xFBaudio"),
        );
    }

    #[test]
    fn mp4_json_run_removes_udta_meta_and_keeps_samples() {
        let tags = atom(
            b"udta",
            &atom(
                b"meta",
                &[
                    vec![0; 4],
                    atom(b"ilst", &atom(b"\xA9nam", b"Secret title")),
                ]
                .concat(),
            ),
        );
        let original = [
            atom(b"ftyp", b"isom\0\0\x02\0isomiso2"),
            atom(b"mdat", b"FRAMEDATA"),
            atom(b"moov", &tags),
        ]
        .concat();
        check_clean("clip.mp4", &original, |clean| {
            assert_eq!(clean.len(), original.len());
            assert!(!clean.windows(12).any(|w| w == b"Secret title"));
            assert!(clean.windows(9).any(|w| w == b"FRAMEDATA"));
        });
    }

    #[test]
    fn mixed_results_report_order_progress_counts_and_first_error() {
        let dir = tempfile::tempdir().unwrap();
        let names = ["bad.flv", "song.mp3", "bad.ts", "again.aac"];
        let mut input = args(&["--json", "--lang", "pl"]);
        for name in names {
            let path = dir.path().join(name);
            std::fs::write(&path, b"audio").unwrap();
            input.push(path.into_os_string());
        }
        let mut output = Vec::new();
        assert_eq!(run(input, &mut output), 0);
        let rows = lines(&output);
        assert_eq!(rows.len(), 5);
        for (index, name) in names.iter().enumerate() {
            assert_eq!(
                rows[index],
                json!({"progress": index as f64 / 4.0, "step": format!("{} / 4: {name}", index + 1)})
            );
        }
        assert_eq!(
            rows[4],
            json!({"summary": "Zapisano czyste kopie: 2. Pierwsza: song-clean.mp3 Niepowodzenia: 2. Pierwszy błąd: bad.flv: nieobsługiwany format"})
        );
    }

    #[test]
    fn all_failures_emit_first_localized_error_and_no_finished_copy() {
        let dir = tempfile::tempdir().unwrap();
        let mut input = args(&["--json", "--lang", "pl"]);
        for name in [
            "clip.flv",
            "clip.ogv",
            "clip.ts",
            "photo.heic",
            "photo.avif",
        ] {
            let path = dir.path().join(name);
            let data = if name.starts_with("photo") {
                [atom(b"ftyp", b"heic\0\0\0\0mif1"), atom(b"meta", b"items")].concat()
            } else {
                b"unsupported".to_vec()
            };
            std::fs::write(&path, data).unwrap();
            input.push(path.into_os_string());
        }
        let mut output = Vec::new();
        assert_eq!(run(input, &mut output), 1);
        let rows = lines(&output);
        assert_eq!(rows.len(), 6);
        assert_eq!(rows[5], json!({"error": "clip.flv: nieobsługiwany format"}));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 5);
    }

    #[test]
    fn plain_text_success_has_step_and_summary() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("song.mp3");
        std::fs::write(&path, b"audio").unwrap();
        let mut output = Vec::new();
        assert_eq!(run(vec![path.into_os_string()], &mut output), 0);
        assert_eq!(
            String::from_utf8(output).unwrap(),
            "1 / 1: song.mp3\nClean copies saved: 1. First: song-clean.mp3\n"
        );
    }

    #[test]
    fn each_json_line_is_flushed_and_broken_output_stops_before_cleaning() {
        #[derive(Default)]
        struct Sink {
            bytes: Vec<u8>,
            flushes: usize,
        }
        impl Write for Sink {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                self.bytes.write(bytes)
            }
            fn flush(&mut self) -> io::Result<()> {
                self.flushes += 1;
                Ok(())
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("song.mp3");
        std::fs::write(&path, b"audio").unwrap();
        let mut sink = Sink::default();
        assert_eq!(
            run(
                vec!["--json".into(), path.clone().into_os_string()],
                &mut sink
            ),
            0
        );
        assert_eq!(sink.flushes, lines(&sink.bytes).len());
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> io::Result<()> {
                Err(io::ErrorKind::BrokenPipe.into())
            }
        }
        assert_eq!(run(vec!["--json".into(), path.into_os_string()], Broken), 1);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    }
}
