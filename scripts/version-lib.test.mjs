import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import test from 'node:test';
import { digestSources } from './version-lib.mjs';

function fixture(t, newline = '\n') {
  const directory = mkdtempSync(join(tmpdir(), 'movemgr-version-test-'));
  t.after(() => {
    const absolute = resolve(directory);
    assert.equal(dirname(absolute), resolve(tmpdir()));
    assert.ok(basename(absolute).startsWith('movemgr-version-test-'));
    rmSync(absolute, { recursive: true, force: true });
  });
  mkdirSync(join(directory, 'src'));
  mkdirSync(join(directory, 'scripts'));
  const texts = {
    '.gitattributes': '* text=auto eol=lf\n*.ps1 text eol=crlf\n',
    'src/main.ts': "export const label = '파일 이동';\nexport const ready = true;\n",
    'scripts/build-portable.ps1': "$ErrorActionPreference = 'Stop'\nWrite-Output 'MoveMgr'\n"
  };
  for (const [path, value] of Object.entries(texts)) {
    writeFileSync(join(directory, path), value.replaceAll('\n', newline), 'utf8');
  }
  return directory;
}

test('source digests are identical for LF and CRLF checkouts', (t) => {
  assert.equal(digestSources(fixture(t)), digestSources(fixture(t, '\r\n')));
});

test('source content changes still invalidate the digest', (t) => {
  const directory = fixture(t);
  const before = digestSources(directory);
  writeFileSync(join(directory, 'src/main.ts'), 'export const ready = false;\n', 'utf8');
  assert.notEqual(digestSources(directory), before);
});

test('binary assets retain their exact bytes in the digest', (t) => {
  const first = fixture(t);
  const second = fixture(t);
  writeFileSync(join(first, 'src/image.png'), Buffer.from([0, 13, 10, 255]));
  writeFileSync(join(second, 'src/image.png'), Buffer.from([0, 10, 255]));
  assert.notEqual(digestSources(first), digestSources(second));
});
