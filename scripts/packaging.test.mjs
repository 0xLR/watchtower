import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const json = path => JSON.parse(readFileSync(new URL('../' + path, import.meta.url), 'utf8'));
test('default packaging never downloads or bundles sidecars', () => {
  const base = json('apps/desktop/src-tauri/tauri.conf.json');
  assert.deepEqual(base.bundle.externalBin, []);
  assert.equal(base.bundle.resources['licenses/mediamtx.txt'], undefined);
  assert.doesNotMatch(json('package.json').scripts.package, /sidecar|fetch-/);
});
for (const [profile, engines] of [['go2rtc', ['go2rtc']], ['mediamtx', ['mediamtx']], ['all-engines', ['go2rtc', 'mediamtx']]]) {
  test(`${profile} keeps capabilities and bundled executables in sync`, () => {
    const overlay = json(`apps/desktop/src-tauri/tauri.${profile}.conf.json`);
    assert.deepEqual(overlay.build.features, engines);
    assert.deepEqual(overlay.bundle.externalBin, engines.map(engine => `binaries/${engine}`));
    const script = json('package.json').scripts[`package:${profile}`];
    for (const engine of ['go2rtc', 'mediamtx']) assert.equal(script.includes(`fetch-${engine}.mjs`), engines.includes(engine));
  });
}
