# WindowCraftExtensions

Extensions for [WindowCraft](https://github.com/windowcraftdev/WindowCraft). Each extension is a separate Windows program written in Rust, built here and published as a GitHub release of this repository.

WindowCraft 1.5.0 reads [index.json](index.json) from this repository, lists the extensions on its Extensions page, and installs the one you pick after checking its SHA-256.

| Extension | What it does |
| --- | --- |
| [metadata-stripper](extensions/metadata-stripper) | Removes GPS location, camera data, dates and other metadata from photos, audio and video |

## How WindowCraft runs an extension

The `ui` block of `extension.json` adds a button to the extension's row. The button opens a file picker, and WindowCraft then runs:

```
<entry> --json --lang <tag> -- <file>...
```

The extension prints one JSON object per line: `{"progress": 0.5, "step": "1 / 2: photo.jpg"}` while it works, then `{"summary": "..."}` with exit code 0, or `{"error": "..."}` with exit code 1. `<tag>` is the app language (en, pl, de, fr, es, it, pt-BR or uk).

## Adding an extension

You need Rust and Node 20 or newer.

1. Copy `template/` to `extensions/<id>/`. The id uses lowercase letters, digits and hyphens.
2. Set the name in `Cargo.toml` and fill in `extension.json`.
3. Build with `cargo build --release -p <id>`.
4. Run `node scripts/build-index.mjs` to check the manifest and update `index.json`.
5. Commit, including `Cargo.lock`, and open a pull request.

## Releasing

1. Raise the version in `Cargo.toml` and `extension.json`, run the script again and commit.
2. Push the tag `<id>-v<version>`. The release workflow builds the x64 and ARM64 exe files and prints their SHA-256.
3. Add those links and hashes under `downloads` in `extension.json`, run the script and commit.

The rules for every manifest field are in [schema/extension.schema.json](schema/extension.schema.json).

## License

MIT, see [LICENSE](LICENSE). An extension can name its own license in its manifest.

Made by Nordan
