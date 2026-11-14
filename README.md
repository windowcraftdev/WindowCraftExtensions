# WindowCraftExtensions

WindowCraftExtensions ([github.com/windowcraftdev/WindowCraftExtensions](https://github.com/windowcraftdev/WindowCraftExtensions)) holds the extensions for [WindowCraft](https://github.com/windowcraftdev/WindowCraft) ([windowcraft.dev](https://windowcraft.dev)), a native Windows 11 app written in Rust. An extension is a standalone Windows program, normally written in Rust, that WindowCraft downloads from a GitHub release of this repository.

## How WindowCraft uses this repository

WindowCraft 1.5.0 has an Extensions page with a button that downloads `index.json` from the `main` branch of this repository and shows the list. For each extension, the page shows the name, version, summary, author, license, and a warning when `min_app` is newer than the running app.

WindowCraft 1.5.0 only shows the list; it does not download or run extensions yet. Installing extensions (download, SHA-256 check, and run) comes in a later WindowCraft version.

WindowCraft drops any index entry that breaks a rule, so a broken manifest makes the extension disappear from the list instead of breaking the list.

## Repository layout

```text
Cargo.toml                      Workspace config (members extensions/*, template excluded, release profile)
index.json                      Generated extension list read by WindowCraft
LICENSE                         MIT license for the repository
README.md                       Repository documentation
extensions/                     Holds extension crates in extensions/<id>/ folders
schema/extension.schema.json    JSON Schema (draft 2020-12) for extensions/<id>/extension.json
schema/index.schema.json        JSON Schema (draft 2020-12) for index.json
scripts/build-index.mjs         Validates manifests, writes or checks index.json, and hashes zip archives
template/Cargo.toml             Starter Cargo package manifest (excluded from the workspace)
template/extension.json         Starter extension manifest validated by scripts/build-index.mjs
template/src/main.rs            Starter binary source file
template/README.md              Note on copying the template folder
.github/workflows/validate.yml  Checks index.json and builds all extensions and template/
.github/workflows/release.yml   Builds x64 and ARM64 binaries and publishes a GitHub release on <id>-v<version> tags
```

The root `Cargo.toml` configures a Rust 2024 workspace with `resolver = "3"`. Its release profile sets `opt-level = 3`, `lto = true`, `codegen-units = 1`, `strip = true`, and `panic = "abort"`.

Running `node scripts/build-index.mjs` validates `template/extension.json` and every `extensions/<id>/extension.json`, then writes `index.json` sorted by `id`. Running `node scripts/build-index.mjs --check` performs the same validation and exits with code 1 if `index.json` is out of date. Running `node scripts/build-index.mjs --hash <zip>` prints the lowercase hex SHA-256 hash of `<zip>`.

## Making an extension

Node 20 or newer is required to run `scripts/build-index.mjs`.

1. Copy `template/` to `extensions/<id>/`, where `<id>` matches `^[a-z0-9][a-z0-9-]{0,47}$`. Do not copy `template/Cargo.lock` or `template/target/` if they exist locally.
2. In `extensions/<id>/Cargo.toml`, set `package.name` and `[[bin]].name` to `<id>`.
3. In `extensions/<id>/extension.json`, set every field for the extension. The `id` field must equal the `<id>` folder name.
4. Write the extension program in `extensions/<id>/src/main.rs`.
5. Build the extension from the repository root with `cargo build --release -p <id>`.
6. Commit the root `Cargo.lock` file, because CI builds workspace crates with `--locked`.
7. Run `node scripts/build-index.mjs` from the repository root to validate manifests and update `index.json`.
8. Commit the changes and open a pull request.

## Publishing a release

1. Bump `version` in `extensions/<id>/Cargo.toml` and `extensions/<id>/extension.json`, make sure `entry` is set in `extensions/<id>/extension.json`, run `cargo build --release -p <id>` and `node scripts/build-index.mjs`, and commit the updated files.
2. Push a tag named `<id>-v<version>`, for example `my-extension-v0.1.0`.
3. Wait for the `Release extension` workflow in `.github/workflows/release.yml` to finish. It checks that the tag matches `extensions/<id>/extension.json` and that `node scripts/build-index.mjs --check` passes, builds `x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc` with `cargo build -p <id> --release --locked`, zips the executable named in `entry` as `<id>-x64.zip` and `<id>-arm64.zip`, uploads both zip files with `SHA256SUMS` to the GitHub release, and prints both SHA-256 hashes in the workflow log.
4. Add the `downloads` block to `extensions/<id>/extension.json` with the release download URLs and the SHA-256 hashes from the release.
5. Run `node scripts/build-index.mjs` again and commit the updated `extensions/<id>/extension.json` and `index.json` to `main`.

The hashes are added after pushing the tag because the manifest at the tag cannot hold the hash of a build made from that tag. MSVC builds are not byte-identical between machines, so the hash from the GitHub release build is the one that counts.

## Manifest reference

Every extension has a manifest at `extensions/<id>/extension.json`. When `node scripts/build-index.mjs` writes `index.json`, it validates each manifest and copies all fields except `schema` into the `extensions` array in `index.json`.

| Field | Required | Rule |
| --- | --- | --- |
| `schema` | yes | Integer `1`. |
| `id` | yes | String matching `^[a-z0-9][a-z0-9-]{0,47}$`. Must equal the folder name under `extensions/` and be unique. |
| `version` | yes | String in `x.y.z` form (`^[0-9]+\.[0-9]+\.[0-9]+$`). |
| `name` | yes | Object keyed by language tag. `en` is required. Each string is 1 to 48 characters after trimming. |
| `summary` | yes | Object keyed by language tag. `en` is required. Each string is 1 to 280 characters after trimming. |
| `author` | yes | String of 1 to 48 characters after trimming. |
| `license` | yes | SPDX license identifier string of 1 to 64 characters after trimming. |
| `min_app` | no | String in `x.y.z` form (`^[0-9]+\.[0-9]+\.[0-9]+$`). |
| `homepage` | no | String starting with `https://github.com/` with no spaces. |
| `entry` | no | File name ending in `.exe` with no `/`, `\`, or `..`. Required when `downloads` is present and when tagging a release. |
| `downloads` | no | Object with keys `x64` and `arm64`. Each entry has `url` under `https://github.com/windowcraftdev/WindowCraftExtensions/releases/download/` and `sha256` as 64 lowercase hex characters (`^[0-9a-f]{64}$`). |

Supported language tags for `name` and `summary` are `en`, `pl`, `de`, `fr`, `es`, `it`, `pt-BR`, and `uk`. Text fields (`name`, `summary`, `author`, and `license`) must not contain control characters (`U+0000` to `U+001F` and `U+007F` to `U+009F`). Unknown fields are rejected by `scripts/build-index.mjs`.

## Example manifest

The JSON below takes `template/extension.json` and adds a `downloads` block. The two `sha256` hashes in this block are example values.

```json
{
  "schema": 1,
  "id": "my-extension",
  "version": "0.1.0",
  "name": {
    "en": "My extension",
    "pl": "Moje rozszerzenie"
  },
  "summary": {
    "en": "A starting point for a WindowCraft extension."
  },
  "author": "Your name",
  "license": "MIT",
  "min_app": "1.5.0",
  "homepage": "https://github.com/windowcraftdev/WindowCraftExtensions/tree/main/extensions/my-extension",
  "entry": "my-extension.exe",
  "downloads": {
    "x64": {
      "url": "https://github.com/windowcraftdev/WindowCraftExtensions/releases/download/my-extension-v0.1.0/my-extension-x64.zip",
      "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    "arm64": {
      "url": "https://github.com/windowcraftdev/WindowCraftExtensions/releases/download/my-extension-v0.1.0/my-extension-arm64.zip",
      "sha256": "01ba4719c80b6fe911b091a7c05124b64eeece964e09c058ef8f9805daca546b"
    }
  }
}
```

## Checks

`.github/workflows/validate.yml` runs on every push and pull request. The `index` job runs on `ubuntu-latest` with Node 20 and executes `node scripts/build-index.mjs --check` to validate `template/extension.json` and every `extensions/<id>/extension.json`, check for duplicate IDs, and confirm that `index.json` matches the manifests.

The `build` job runs on `windows-latest` after `index` passes. When at least one folder under `extensions/` has a `Cargo.toml`, it runs `cargo build --workspace --release --locked`. It then runs `cargo build --release` in `template/`.

## License

This repository is licensed under the MIT License; see [LICENSE](LICENSE). Each extension may name its own license in its `extension.json` manifest.

Made by Nordan
