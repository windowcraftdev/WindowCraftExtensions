// Made by Nordan
//! A starting point: WindowCraft runs `my-extension.exe --json --lang <tag> -- <file>...`
//! and reads one JSON object per line from stdout.

use std::io::Write;

fn main() {
    let mut json = false;
    let mut files = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" => json = true,
            // The language tag; this template answers in English only.
            "--lang" => drop(args.next()),
            "--" => files.extend(args.by_ref()),
            _ => files.push(arg),
        }
    }
    let mut out = std::io::stdout().lock();
    if files.is_empty() {
        line(&mut out, json, "error", "Choose at least one file.");
        std::process::exit(1);
    }
    for (index, file) in files.iter().enumerate() {
        let step = format!("{} / {}: {file}", index + 1, files.len());
        if json {
            let share = index as f64 / files.len() as f64;
            let _ = writeln!(out, "{{\"progress\": {share}, \"step\": \"{}\"}}", escape(&step));
            let _ = out.flush();
        } else {
            let _ = writeln!(out, "{step}");
        }
        // The work on `file` goes here.
    }
    line(&mut out, json, "summary", &format!("Files checked: {}", files.len()));
}

fn line(out: &mut impl Write, json: bool, key: &str, text: &str) {
    if json {
        let _ = writeln!(out, "{{\"{key}\": \"{}\"}}", escape(text));
    } else {
        let _ = writeln!(out, "{text}");
    }
    let _ = out.flush();
}

/// Escapes a string for JSON.
fn escape(text: &str) -> String {
    let mut escaped = String::new();
    for c in text.chars() {
        match c {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            c if c.is_control() => escaped.push_str(&format!("\\u{:04x}", c as u32)),
            c => escaped.push(c),
        }
    }
    escaped
}
