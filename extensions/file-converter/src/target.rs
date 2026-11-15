// Made by Nordan
use std::path::{Path, PathBuf};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Image,
    Audio,
    Video,
    Data,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Png,
    Jpg,
    Webp,
    Bmp,
    Tiff,
    Gif,
    Ico,
    Qoi,
    Avif,
    Mp3,
    Flac,
    Wav,
    Aiff,
    M4a,
    Opus,
    Mp4,
    Mov,
    Mkv,
    Webm,
    Json,
    Yaml,
    Toml,
    Csv,
    Html,
}
impl Target {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "png" => Some(Self::Png),
            "jpg" => Some(Self::Jpg),
            "webp" => Some(Self::Webp),
            "bmp" => Some(Self::Bmp),
            "tiff" => Some(Self::Tiff),
            "gif" => Some(Self::Gif),
            "ico" => Some(Self::Ico),
            "qoi" => Some(Self::Qoi),
            "avif" => Some(Self::Avif),
            "mp3" => Some(Self::Mp3),
            "flac" => Some(Self::Flac),
            "wav" => Some(Self::Wav),
            "aiff" => Some(Self::Aiff),
            "m4a" => Some(Self::M4a),
            "opus" => Some(Self::Opus),
            "mp4" => Some(Self::Mp4),
            "mov" => Some(Self::Mov),
            "mkv" => Some(Self::Mkv),
            "webm" => Some(Self::Webm),
            "json" => Some(Self::Json),
            "yaml" => Some(Self::Yaml),
            "toml" => Some(Self::Toml),
            "csv" => Some(Self::Csv),
            "html" => Some(Self::Html),
            _ => None,
        }
    }
    pub fn value(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpg => "jpg",
            Self::Webp => "webp",
            Self::Bmp => "bmp",
            Self::Tiff => "tiff",
            Self::Gif => "gif",
            Self::Ico => "ico",
            Self::Qoi => "qoi",
            Self::Avif => "avif",
            Self::Mp3 => "mp3",
            Self::Flac => "flac",
            Self::Wav => "wav",
            Self::Aiff => "aiff",
            Self::M4a => "m4a",
            Self::Opus => "opus",
            Self::Mp4 => "mp4",
            Self::Mov => "mov",
            Self::Mkv => "mkv",
            Self::Webm => "webm",
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Toml => "toml",
            Self::Csv => "csv",
            Self::Html => "html",
        }
    }
    pub fn extension(self) -> &'static str {
        if self == Self::Tiff {
            "tif"
        } else {
            self.value()
        }
    }
    pub fn family(self) -> Family {
        match self {
            Self::Png
            | Self::Jpg
            | Self::Webp
            | Self::Bmp
            | Self::Tiff
            | Self::Gif
            | Self::Ico
            | Self::Qoi
            | Self::Avif => Family::Image,
            Self::Mp3 | Self::Flac | Self::Wav | Self::Aiff | Self::M4a | Self::Opus => {
                Family::Audio
            }
            Self::Mp4 | Self::Mov | Self::Mkv | Self::Webm => Family::Video,
            Self::Json | Self::Yaml | Self::Toml | Self::Csv | Self::Html => Family::Data,
        }
    }
}
pub fn output_path(source: &Path, target: Target) -> PathBuf {
    let stem = source.file_stem().map_or_else(
        || "converted".into(),
        |stem| stem.to_string_lossy().into_owned(),
    );
    let dir = source.parent().unwrap_or_else(|| Path::new("."));
    let mut candidate = dir.join(format!("{stem}.{}", target.extension()));
    let mut number = 2;
    while candidate.exists() {
        candidate = dir.join(format!("{stem} ({number}).{}", target.extension()));
        number += 1;
    }
    candidate
}
