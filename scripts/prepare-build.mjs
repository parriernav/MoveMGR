import { digestSources, readManifest, syncVersion, writeManifest } from './version-lib.mjs';

const manifest = readManifest();
const digest = digestSources();
if (manifest.sourceDigest && manifest.sourceDigest !== digest) {
  const [major, minor, patch] = manifest.version.split('.').map(Number);
  manifest.version = `${major}.${minor}.${patch + 1}`;
}
manifest.sourceDigest = digest;
syncVersion(manifest.version);
writeManifest(manifest);
console.log(`MoveMgr ${manifest.version} (${digest.slice(0, 12)})`);

