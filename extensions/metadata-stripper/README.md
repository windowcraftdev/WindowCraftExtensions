# Metadata stripper

Writes a copy of each file without GPS location, camera data, dates, author, comments and thumbnails. The original is not changed; the copy gets `-clean` in its name, for example `photo-clean.jpg`. Everything is done in Rust on the computer, with no other program and no network access.

Supported: JPEG, PNG, WebP, GIF, MP3, AAC, FLAC, MP4, MOV, M4A, 3GP, MKV, WebM, WAV, AVI, AIFF, Ogg, Opus, WMA and WMV. JPEG keeps its color profile and orientation. FLV, OGV, TS and HEIF files are not supported.

Install it from the Extensions page of WindowCraft, or run it by hand:

```
metadata-stripper.exe photo.jpg song.mp3
```

Made by Nordan
