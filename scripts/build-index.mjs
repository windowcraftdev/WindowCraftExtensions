// Made by Nordan
import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { readFile, readdir, stat, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const fields = ['schema', 'id', 'version', 'name', 'summary', 'author', 'license', 'min_app', 'homepage', 'entry', 'ui', 'downloads'];
// Texts of `ui`: maximum length and whether it is required.
const uiTexts = { title: [80, true], button: [32, true], note: [280, false], dialog: [80, true], files: [60, true] };
const languages = ['en', 'pl', 'de', 'fr', 'es', 'it', 'pt-BR', 'uk'];
const releasePrefix = 'https://github.com/windowcraftdev/WindowCraftExtensions/releases/download/';
const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const matches = (value, pattern) => typeof value === 'string' && pattern.test(value);
const end = '(?![\\s\\S])';
const versionPattern = new RegExp('^[0-9]+\\.[0-9]+\\.[0-9]+' + end);
const idPattern = new RegExp('^[a-z0-9][a-z0-9-]{0,47}' + end);
const hashPattern = new RegExp('^[0-9a-f]{64}' + end);
const patternsPattern = new RegExp('^\\*\\.[a-z0-9]{1,10}(;\\*\\.[a-z0-9]{1,10})*' + end);

// A text by language tag: en is required, each value 1 to `limit` characters.
function checkLocalized(key, localized, limit, report) {
  if (!object(localized)) {
    report(`${key} must be an object with an en value`);
    return;
  }
  if (!Object.hasOwn(localized, 'en')) report(`${key}.en is required`);
  for (const [language, value] of Object.entries(localized)) {
    if (!languages.includes(language)) report(`${key}.${language} is not a supported language`);
    if (typeof value !== 'string' || Array.from(value.trim()).length < 1 || Array.from(value.trim()).length > limit) {
      report(`${key}.${language} must have 1 to ${limit} characters after trimming`);
    }
    if (typeof value === 'string' && /[\u0000-\u001f\u007f-\u009f]/u.test(value)) {
      report(`${key}.${language} must not contain control characters`);
    }
  }
}

// The row WindowCraft draws for an installed extension: a button that opens
// a file picker and runs the extension on the chosen files.
function checkUi(ui, report) {
  if (!object(ui)) {
    report('ui must be an object');
    return;
  }
  for (const key of Object.keys(ui)) {
    if (!Object.hasOwn(uiTexts, key) && key !== 'patterns') report(`ui has unknown property "${key}"`);
  }
  for (const [key, [limit, required]] of Object.entries(uiTexts)) {
    if (Object.hasOwn(ui, key)) checkLocalized(`ui.${key}`, ui[key], limit, report);
    else if (required) report(`ui.${key} is required`);
  }
  if (!matches(ui.patterns, patternsPattern) || ui.patterns.length > 1000) {
    report('ui.patterns must look like *.jpg;*.png (lowercase extensions, at most 1000 characters)');
  }
}

function validate(manifest, folder, report) {
  if (!object(manifest)) {
    report('manifest must be an object');
    return;
  }
  for (const key of Object.keys(manifest)) {
    if (!fields.includes(key)) report(`unknown property "${key}"`);
  }
  if (manifest.schema !== 1) report('schema must be the integer 1');
  if (!matches(manifest.id, idPattern)) report('id must match ^[a-z0-9][a-z0-9-]{0,47}$');
  if (folder !== undefined && manifest.id !== folder) report(`id must equal folder name "${folder}"`);
  if (!matches(manifest.version, versionPattern)) report('version must have the form 0.1.0');
  checkLocalized('name', manifest.name, 48, report);
  checkLocalized('summary', manifest.summary, 280, report);
  if (Object.hasOwn(manifest, 'ui')) {
    if (!Object.hasOwn(manifest, 'entry')) report('entry is required when ui is present');
    checkUi(manifest.ui, report);
  }
  for (const [key, limit] of [['author', 48], ['license', 64]]) {
    const value = manifest[key];
    if (typeof value !== 'string' || Array.from(value.trim()).length < 1 || Array.from(value.trim()).length > limit) {
      report(`${key} must have 1 to ${limit} characters after trimming`);
    }
    if (typeof value === 'string' && /[\u0000-\u001f\u007f-\u009f]/u.test(value)) {
      report(`${key} must not contain control characters`);
    }
  }
  if (Object.hasOwn(manifest, 'min_app') && !matches(manifest.min_app, versionPattern)) report('min_app must have the form 1.5.0');
  if (Object.hasOwn(manifest, 'homepage') && (typeof manifest.homepage !== 'string' || !manifest.homepage.startsWith('https://github.com/') || /\s/u.test(manifest.homepage))) {
    report('homepage must start with https://github.com/ and have no spaces');
  }
  if (Object.hasOwn(manifest, 'entry') && (typeof manifest.entry !== 'string' || !manifest.entry.endsWith('.exe') || /[/\\]|\.\./u.test(manifest.entry))) {
    report('entry must be a file name ending in .exe without /, \\ or ..');
  }
  if (Object.hasOwn(manifest, 'downloads')) {
    if (!Object.hasOwn(manifest, 'entry')) report('entry is required when downloads is present');
    if (!object(manifest.downloads)) {
      report('downloads must be an object');
    } else {
      if (Object.keys(manifest.downloads).length === 0) report('downloads must have x64, arm64 or both');
      for (const [arch, download] of Object.entries(manifest.downloads)) {
        if (!['x64', 'arm64'].includes(arch)) report(`downloads.${arch} is not a supported architecture`);
        if (!object(download)) {
          report(`downloads.${arch} must be an object`);
          continue;
        }
        for (const key of Object.keys(download)) {
          if (!['url', 'sha256'].includes(key)) report(`downloads.${arch} has unknown property "${key}"`);
        }
        // The release workflow names the files this way; WindowCraft saves
        // the download as `entry`, so the URL must point at an exe.
        const url = `${releasePrefix}${manifest.id}-v${manifest.version}/${manifest.id}-${arch}.exe`;
        if (download.url !== url) report(`downloads.${arch}.url must be ${url}`);
        if (!matches(download.sha256, hashPattern)) report(`downloads.${arch}.sha256 must be 64 lowercase hex characters`);
      }
    }
  }
}

async function exists(path) {
  try {
    await stat(path);
    return true;
  } catch (error) {
    if (error.code === 'ENOENT') return false;
    throw error;
  }
}

async function main() {
  const args = process.argv.slice(2);
  if (args[0] === '--hash' && args.length === 2) {
    const hash = createHash('sha256');
    for await (const chunk of createReadStream(args[1])) hash.update(chunk);
    console.log(hash.digest('hex'));
    return;
  }
  if (args.length > 1 || (args.length === 1 && args[0] !== '--check')) {
    throw new Error('Usage: node scripts/build-index.mjs [--check | --hash <zip>]');
  }
  let problems = 0;
  const entries = [];
  const ids = new Set();
  const folders = (await readdir(join(root, 'extensions'), { withFileTypes: true }))
    .filter(item => item.isDirectory()).map(item => item.name).sort();
  for (const folder of [...folders, null]) {
    const path = folder === null ? 'template/extension.json' : `extensions/${folder}/extension.json`;
    const report = problem => {
      console.error(`${path}: ${problem}`);
      problems++;
    };
    if (!(await exists(join(root, path)))) {
      if (folder === null || await exists(join(root, 'extensions', folder, 'Cargo.toml'))) report('manifest is missing');
      continue;
    }
    let manifest;
    try {
      manifest = JSON.parse(await readFile(join(root, path), 'utf8'));
    } catch (error) {
      report(`cannot read JSON: ${error.message}`);
      continue;
    }
    validate(manifest, folder ?? undefined, report);
    if (folder !== null && object(manifest)) {
      if (ids.has(manifest.id)) report(`duplicate id "${manifest.id}"`);
      ids.add(manifest.id);
      entries.push(Object.fromEntries(fields.slice(1).filter(key => Object.hasOwn(manifest, key)).map(key => [key, manifest[key]])));
    }
  }
  if (problems) {
    process.exitCode = 1;
    return;
  }
  entries.sort((a, b) => a.id < b.id ? -1 : a.id > b.id ? 1 : 0);
  const expected = JSON.stringify({ schema: 1, extensions: entries }, null, 2) + '\n';
  const indexPath = join(root, 'index.json');
  if (args[0] === '--check') {
    const actual = await exists(indexPath) ? await readFile(indexPath, 'utf8') : null;
    if (actual !== expected) throw new Error('index.json is out of date. Run node scripts/build-index.mjs');
  } else {
    await writeFile(indexPath, expected, 'utf8');
  }
}

main().catch(error => {
  console.error(error.message);
  process.exitCode = 1;
});
