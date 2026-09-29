import { digestSources, readManifest, syncVersion, validVersion, writeManifest } from './version-lib.mjs';

const requested = process.argv[2];
if (!validVersion(requested ?? '')) {
  console.error('사용법: pnpm version:set 0.1.0');
  process.exit(1);
}
const manifest = readManifest();
manifest.version = requested;
syncVersion(requested);
manifest.sourceDigest = digestSources();
writeManifest(manifest);
console.log(`MoveMgr 버전을 ${requested}(으)로 설정했습니다.`);

