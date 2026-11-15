# File converter

Converts pictures, sound, video and data files. Everything runs on this computer: the exe has no ffmpeg inside, downloads nothing and opens no network connection.

| Group | Target (`--to`) | Reads |
| --- | --- | --- |
| Pictures | `png`, `jpg`, `webp`, `avif`, `bmp`, `tiff`, `gif`, `ico`, `qoi` | PNG, JPG, JPEG, JFIF, WebP, GIF, BMP, TIF, TIFF, ICO, TGA, QOI, PPM, PGM, PBM, PNM, HDR, EXR, DDS, Farbfeld (`.ff`) |
| Audio | `mp3`, `flac`, `wav`, `aiff` | MP3, FLAC, WAV, AIF, AIFF, M4A, AAC, OGG, OGA, CAF, MKA, and the sound of MP4, M4V, MOV, MKV, WebM, 3GP |
| Audio | `m4a` | AAC or ALAC sound from MP4, M4V, MOV, M4A, MKV, MKA, WebM, 3GP |
| Audio | `opus` | Opus sound from WebM, MKV, MKA, MP4, M4A, MOV |
| Video | `mp4`, `mov`, `mkv`, `webm` | MP4, M4V, MOV, MKV, WebM, 3GP |
| Data | `json` | JSON, YAML, YML, TOML, CSV |
| Data | `yaml`, `toml` | JSON, YAML, YML, TOML |
| Data | `csv` | JSON, YAML, YML |
| Data | `html` | Markdown (`.md`, `.markdown`) |

Video, M4A and Opus are stream copies: nothing is re-encoded, so the quality stays the same and the work is as fast as the disk. The streams must already fit the target container. VP9 cannot go into MOV, Vorbis cannot go into MP4, and a file with such a stream is refused. WebM cannot hold subtitles, so they are left out of a WebM copy. M4A and Opus take the first audio track that fits.

MP3 is written by LAME at V0, the highest variable bitrate setting, with the length in its header so players can seek. FLAC keeps 24 bits from deeper sources. Surround sound is folded to stereo for MP3.

JPG output puts transparent pixels on white. WebP output is lossless. AVIF output uses quality 80. ICO output is scaled down to 256 px. An animated GIF is refused for GIF and WebP output, so no frames are lost; for other targets its first frame is used.

A CSV is written with a UTF-8 BOM, so Excel shows letters such as ł and ü correctly. A CSV read as JSON gives every value as text. TOML output needs a table at the top and no null values. Markdown is turned into one HTML page; HTML written inside the Markdown is shown as text and does not run.

The new file is saved next to the original as `name.ext`, or `name (2).ext` when that name is taken. The original stays as it is. A file that already is in the target format is skipped with a message.

## Command line

```
file-converter.exe --json --lang pl --to mp3 -- song.flac
```

`--to` takes a value from the Target column. With `--json` the program prints one JSON object per line, as WindowCraft expects; without it, plain lines.

## License

The code is MIT. MP3 files are written by LAME (https://lame.sourceforge.io), which is LGPL 2.0 or later and is linked into the exe through the `mp3lame-encoder` crate. Its source is on that site, and the rest of this extension can be rebuilt from this repository, so LAME can be swapped for another build.

Made by Nordan
