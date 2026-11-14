# WindowCraftExtensions

Extensions for [WindowCraft](https://github.com/windowcraftdev/WindowCraft). Each extension is a separate Windows program, built here and published as a GitHub release of this repository.

WindowCraft 1.5.0 downloads [index.json](index.json) from this repository and shows the list on its Extensions page. Installing extensions from the app comes in a later version.

## Adding an extension

You need Rust and Node 20 or newer.

1. Copy `template/` to `extensions/<id>/`. The id uses lowercase letters, digits and hyphens.
2. Set the name in `Cargo.toml` and fill in `extension.json`.
3. Build with `cargo build --release -p <id>`.
4. Run `node scripts/build-index.mjs` to check the manifest and update `index.json`.
5. Commit, including `Cargo.lock`, and open a pull request.

## Releasing

1. Raise the version in `Cargo.toml` and `extension.json`, run the script again and commit.
2. Push the tag `<id>-v<version>`. The release workflow builds x64 and ARM64 zips and prints their SHA-256.
3. Add those links and hashes under `downloads` in `extension.json`, run the script and commit.

The rules for every manifest field are in [schema/extension.schema.json](schema/extension.schema.json).

## License

MIT, see [LICENSE](LICENSE). An extension can name its own license in its manifest.

Made by Nordan
