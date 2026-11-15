// Made by Nordan
use crate::text::Reason;
use crate::{Error, Result, Target};
use serde_json::Value;
use std::path::Path;

fn invalid(e: impl std::fmt::Display) -> Error {
    Error::Invalid(e.to_string())
}
pub fn convert(source: &Path, output: &Path, target: Target) -> Result<bool> {
    let ext = source
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext == target.value() || (ext == "yml" && target == Target::Yaml) {
        return Err(Error::Already(target));
    }
    let supported = if target == Target::Html {
        matches!(ext.as_str(), "md" | "markdown")
    } else {
        matches!(ext.as_str(), "json" | "yaml" | "yml" | "toml")
            || (ext == "csv" && target == Target::Json)
    };
    if !supported {
        return Ok(false);
    }
    let input =
        std::fs::read_to_string(source).map_err(|e| Error::io(crate::file_name(source), e))?;
    // Notepad and Excel often save UTF-8 with a BOM, which the parsers reject.
    let input = match input.strip_prefix('\u{feff}') {
        Some(rest) => rest.to_owned(),
        None => input,
    };
    let result = if target == Target::Html {
        if !matches!(ext.as_str(), "md" | "markdown") {
            return Ok(false);
        }
        use pulldown_cmark::{Event, Parser, html};
        let events = Parser::new(&input).map(|event| match event {
            Event::Html(s) | Event::InlineHtml(s) => Event::Text(s),
            other => other,
        });
        let mut body = String::new();
        html::push_html(&mut body, events);
        format!(
            "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>{}</title></head><body>\n{body}</body></html>\n",
            escape(&crate::file_name(source))
        )
    } else {
        let value: Value = match ext.as_str() {
            "json" => serde_json::from_str(&input).map_err(invalid)?,
            "yaml" | "yml" => serde_saphyr::from_str(&input).map_err(invalid)?,
            "toml" => {
                let v: toml::Value = toml::from_str(&input).map_err(invalid)?;
                serde_json::to_value(v).map_err(invalid)?
            }
            "csv" if target == Target::Json => {
                let mut reader = csv::Reader::from_reader(input.as_bytes());
                let headers = reader.headers().map_err(invalid)?.clone();
                if headers
                    .iter()
                    .enumerate()
                    .any(|(i, h)| headers.iter().take(i).any(|prev| prev == h))
                {
                    return Err(Error::Reason(Reason::Flat));
                }
                let rows = reader
                    .records()
                    .map(|r| {
                        let r = r.map_err(invalid)?;
                        Ok(Value::Object(
                            headers
                                .iter()
                                .zip(r.iter())
                                .map(|(k, v)| (k.to_owned(), Value::String(v.to_owned())))
                                .collect(),
                        ))
                    })
                    .collect::<Result<Vec<_>>>()?;
                Value::Array(rows)
            }
            _ => return Ok(false),
        };
        match target {
            Target::Json => serde_json::to_string_pretty(&value).map_err(invalid)? + "\n",
            Target::Yaml => serde_saphyr::to_string(&value).map_err(invalid)?,
            Target::Toml => {
                if !value.is_object() || has_null(&value) {
                    return Err(Error::Reason(Reason::Toml));
                }
                toml::to_string_pretty(&value).map_err(invalid)?
            }
            // Excel reads a CSV without a BOM in the local code page, which
            // breaks every letter outside ASCII.
            Target::Csv if matches!(ext.as_str(), "json" | "yaml" | "yml") => {
                format!("\u{feff}{}", csv_text(&value)?)
            }
            _ => return Ok(false),
        }
    };
    std::fs::write(output, result).map_err(|e| Error::io(crate::file_name(output), e))?;
    Ok(true)
}
fn has_null(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Array(a) => a.iter().any(has_null),
        Value::Object(o) => o.values().any(has_null),
        _ => false,
    }
}
fn csv_text(value: &Value) -> Result<String> {
    let rows = value.as_array().ok_or(Error::Reason(Reason::Flat))?;
    let mut keys = Vec::new();
    for row in rows {
        let row = row.as_object().ok_or(Error::Reason(Reason::Flat))?;
        for (key, val) in row {
            if val.is_array() || val.is_object() {
                return Err(Error::Reason(Reason::Flat));
            }
            if !keys.contains(key) {
                keys.push(key.clone());
            }
        }
    }
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer.write_record(&keys).map_err(invalid)?;
    for row in rows {
        writer
            .write_record(keys.iter().map(|key| match &row[key] {
                Value::String(s) => s.clone(),
                Value::Null => String::new(),
                other => other.to_string(),
            }))
            .map_err(invalid)?;
    }
    String::from_utf8(writer.into_inner().map_err(invalid)?).map_err(invalid)
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn structured_round_trip() {
        let d = tempfile::tempdir().unwrap();
        let value =
            serde_json::json!({"name":"sample", "count":3, "nested":{"ok":true}, "list":[1,2]});
        let mut path = d.path().join("in.json");
        std::fs::write(&path, value.to_string()).unwrap();
        for target in [Target::Yaml, Target::Toml, Target::Json] {
            let out = d.path().join(format!("out.{}", target.extension()));
            assert!(convert(&path, &out, target).unwrap());
            path = out;
        }
        assert_eq!(
            serde_json::from_str::<Value>(&std::fs::read_to_string(path).unwrap()).unwrap(),
            value
        );
    }
    #[test]
    fn csv_union_order_and_string_round_trip() {
        let value: Value =
            serde_json::from_str(r#"[{"z":"a,b","a":"line\nnext"},{"q":"\"quoted\"","z":"x"}]"#)
                .unwrap();
        let csv = csv_text(&value).unwrap();
        assert!(csv.starts_with("z,a,q\n"));
        let d = tempfile::tempdir().unwrap();
        let input = d.path().join("in.csv");
        let out = d.path().join("out.json");
        std::fs::write(&input, csv).unwrap();
        assert!(convert(&input, &out, Target::Json).unwrap());
        let back: Value = serde_json::from_slice(&std::fs::read(out).unwrap()).unwrap();
        assert_eq!(back[0]["z"], "a,b");
        assert_eq!(back[0]["a"], "line\nnext");
        assert_eq!(back[1]["q"], "\"quoted\"");
        assert_eq!(
            back,
            serde_json::json!([
                {"z":"a,b","a":"line\nnext","q":""},
                {"z":"x","a":"","q":"\"quoted\""}
            ])
        );
    }

    #[test]
    fn json_csv_json_round_trip_and_yaml_input() {
        let d = tempfile::tempdir().unwrap();
        let input = d.path().join("in.json");
        let value = serde_json::json!([{"name":"Ada","city":"Łódź"},{"name":"Lin","city":"Paris"}]);
        std::fs::write(&input, value.to_string()).unwrap();
        let csv = d.path().join("out.csv");
        let back = d.path().join("back.json");
        assert!(convert(&input, &csv, Target::Csv).unwrap());
        assert!(convert(&csv, &back, Target::Json).unwrap());
        assert_eq!(
            serde_json::from_slice::<Value>(&std::fs::read(back).unwrap()).unwrap(),
            value
        );
        let yaml = d.path().join("in.yaml");
        std::fs::write(&yaml, "- name: Ada\n  count: 2\n- city: Paris\n").unwrap();
        assert!(convert(&yaml, &csv, Target::Csv).unwrap());
        assert_eq!(
            std::fs::read_to_string(csv).unwrap(),
            "\u{feff}name,count,city\nAda,2,\n,,Paris\n"
        );
    }
    #[test]
    fn a_bom_at_the_start_is_skipped() {
        let d = tempfile::tempdir().unwrap();
        let input = d.path().join("in.json");
        std::fs::write(&input, "\u{feff}{\"a\": 1}").unwrap();
        let out = d.path().join("out.yaml");
        assert!(convert(&input, &out, Target::Yaml).unwrap());
        assert_eq!(std::fs::read_to_string(out).unwrap(), "a: 1\n");
    }
    #[test]
    fn markdown_escapes_raw_html_and_title() {
        let d = tempfile::tempdir().unwrap();
        let input = d.path().join("a&b.md");
        let out = d.path().join("out.html");
        std::fs::write(
            &input,
            "# Hello\n\n<script>alert(1)</script>\n\ninline <b>bold</b>",
        )
        .unwrap();
        assert!(convert(&input, &out, Target::Html).unwrap());
        let s = std::fs::read_to_string(out).unwrap();
        assert!(s.contains("<h1>Hello</h1>"));
        assert!(s.contains("a&amp;b.md"));
        assert!(!s.contains("<script>"));
        assert!(!s.contains("<b>"));
        assert!(s.contains("&lt;script&gt;"));
    }
    #[test]
    fn rejects_toml_null_and_non_table_and_nested_csv() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("a.json");
        let out = d.path().join("b.toml");
        for s in ["[1,2]", "{\"a\":[null]}"] {
            std::fs::write(&p, s).unwrap();
            assert!(matches!(
                convert(&p, &out, Target::Toml),
                Err(Error::Reason(Reason::Toml))
            ));
        }
        assert!(csv_text(&serde_json::json!([{"a":{}}])).is_err());
    }
}
