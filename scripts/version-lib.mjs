import { createHash } from 'node:crypto';
import { existsSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { extname, join, relative } from 'node:path';

const root = new URL('../', import.meta.url).pathname.replace(/^\/(?:([A-Za-z]:))/, '$1');
const versionPath = join(root, 'version.json');

function walk(directory) {
  if (!existsSync(directory)) return [];
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return walk(path);
    return [path];
  });
}

export function digestSources() {
  const explicit = ['package.json', 'pnpm-lock.yaml', 'pnpm-workspace.yaml', 'vite.config.ts', 'vitest.config.ts', 'tsconfig.json', '.gitattributes', 'index.html', 'design/movemgr-icon.svg', 'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock', 'src-tauri/tauri.conf.json', 'src-tauri/build.rs'];
  const roots = ['src', 'src-tauri/src', 'src-tauri/capabilities', 'scripts', 'portable', '.github/workflows'];
  const files = [...explicit.map((path) => join(root, path)), ...roots.flatMap((path) => walk(join(root, path)))].filter(existsSync).sort();
  const hash = createHash('sha256');
  for (const file of files) {
    let content = readFileSync(file);
    const rel = relative(root, file).replaceAll('\\', '/');
    if (rel === 'package.json') {
      const json = JSON.parse(content.toString('utf8'));
      json.version = '<VERSION>';
      content = Buffer.from(JSON.stringify(json));
    } else if (rel === 'index.html') {
      content = Buffer.from(content.toString('utf8').replace(/MoveMgr ver\.\d+\.\d+\.\d+/g, 'MoveMgr ver.<VERSION>'));
    } else if (rel === 'src-tauri/Cargo.toml') {
      content = Buffer.from(content.toString('utf8').replace(/(^\[package\][\s\S]*?^version\s*=\s*)"[^"]+"/m, '$1"<VERSION>"'));
    } else if (rel === 'src-tauri/Cargo.lock') {
      content = Buffer.from(content.toString('utf8').replace(/(name = "movemgr"\r?\nversion = )"[^"]+"/, '$1"<VERSION>"'));
    } else if (rel === 'src-tauri/tauri.conf.json') {
      const json = JSON.parse(content.toString('utf8'));
      json.version = '<VERSION>';
      json.app.windows[0].title = 'MoveMgr ver.<VERSION>';
      content = Buffer.from(JSON.stringify(json));
    }
    hash.update(rel, 'utf8');
    hash.update('\0');
    hash.update(content);
    hash.update('\0');
  }
  return hash.digest('hex');
}

export function readManifest() {
  return JSON.parse(readFileSync(versionPath, 'utf8'));
}

export function writeManifest(manifest) {
  writeFileSync(versionPath, `${JSON.stringify(manifest, null, 2)}\n`, 'utf8');
}

export function syncVersion(version) {
  const packagePath = join(root, 'package.json');
  const packageJson = JSON.parse(readFileSync(packagePath, 'utf8'));
  packageJson.version = version;
  writeFileSync(packagePath, `${JSON.stringify(packageJson, null, 2)}\n`, 'utf8');

  const tauriPath = join(root, 'src-tauri/tauri.conf.json');
  const tauri = JSON.parse(readFileSync(tauriPath, 'utf8'));
  tauri.version = version;
  tauri.app.windows[0].title = `MoveMgr ver.${version}`;
  writeFileSync(tauriPath, `${JSON.stringify(tauri, null, 2)}\n`, 'utf8');

  const cargoPath = join(root, 'src-tauri/Cargo.toml');
  const cargo = readFileSync(cargoPath, 'utf8').replace(/(^\[package\][\s\S]*?^version\s*=\s*)"[^"]+"/m, `$1"${version}"`);
  writeFileSync(cargoPath, cargo, 'utf8');

  const indexPath = join(root, 'index.html');
  const index = readFileSync(indexPath, 'utf8').replace(/MoveMgr ver\.\d+\.\d+\.\d+/g, `MoveMgr ver.${version}`);
  writeFileSync(indexPath, index, 'utf8');
}

export function validVersion(version) {
  return /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version);
}
