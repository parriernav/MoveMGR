import { readFileSync } from 'node:fs';
import { digestSources, readManifest } from './version-lib.mjs';

const manifest = readManifest();
const packageVersion = JSON.parse(readFileSync('package.json', 'utf8')).version;
const tauriVersion = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8')).version;
const cargoVersion = readFileSync('src-tauri/Cargo.toml', 'utf8').match(/^version\s*=\s*"([^"]+)"/m)?.[1];
const digest = digestSources();
const errors = [];
if (manifest.sourceDigest !== digest) errors.push('소스 digest가 변경되었습니다. prepare-build를 실행하세요.');
for (const [name, value] of [['package.json', packageVersion], ['tauri.conf.json', tauriVersion], ['Cargo.toml', cargoVersion]]) {
  if (value !== manifest.version) errors.push(`${name} 버전 ${value}가 ${manifest.version}와 다릅니다.`);
}
if (errors.length) { console.error(errors.join('\n')); process.exit(1); }
console.log(`PASS: MoveMgr ${manifest.version}, digest ${digest.slice(0, 12)}`);

