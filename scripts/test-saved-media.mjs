import { spawn, execFileSync } from 'node:child_process';
import { readFile, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { root } from './dev-fixtures.mjs';
const sourceFile = join(root, '.local/saved-streams.json');
if (!(JSON.parse(await readFile(sourceFile, 'utf8'))).length) throw new Error('No authorized credential-free sources prepared');
const triple = execFileSync('rustc', ['-vV'], { encoding: 'utf8' }).match(/^host: (.+)$/m)[1].trim();
const suffix = process.platform === 'win32' ? '.exe' : '';
const file = join(root, 'apps/desktop/e2e/playback.json');
const env = { ...process.env, WATCHTOWER_SAVED_SOURCES: sourceFile, WATCHTOWER_TEST_PLAYBACK: file, WATCHTOWER_TEST_SIDECAR: join(root, `apps/desktop/src-tauri/binaries/go2rtc-${triple}${suffix}`), WATCHTOWER_TEST_MEDIAMTX: join(root, `apps/desktop/src-tauri/binaries/mediamtx-${triple}${suffix}`) };
const run = (program, args) => new Promise((resolve, reject) => { const child = spawn(program, args, { cwd: root, env, stdio: 'inherit', windowsHide: true }); child.once('error', reject); child.once('exit', code => code === 0 ? resolve() : reject(new Error(`Test exited with ${code}`))); });
let gateway;
try {
  await run('cargo', ['build', '-p', 'watchtower-media', '--example', 'fixture']);
  for (const engine of process.env.WATCHTOWER_TEST_ENGINE ? [process.env.WATCHTOWER_TEST_ENGINE] : ['retina', 'go2rtc', 'mediamtx']) {
    env.WATCHTOWER_TEST_ENGINE = engine;
    await rm(file, { force: true });
    gateway = spawn(join(root, `target/debug/examples/fixture${suffix}`), [], { cwd: root, env, stdio: 'inherit', windowsHide: true });
    let ready = false;
    for (let attempt = 0; attempt < 150; attempt++) { if (gateway.exitCode !== null) break; try { await readFile(file); ready = true; break; } catch { await new Promise(resolve => setTimeout(resolve, 200)); } }
    if (!ready) throw new Error(`${engine} saved-stream gateway did not start`);
    if (!process.env.npm_execpath) throw new Error('Run using pnpm test:saved');
    await run(process.execPath, [process.env.npm_execpath, '--filter', '@watchtower/desktop', 'exec', 'playwright', 'test', 'saved.spec.ts']);
    gateway.kill(); gateway = undefined;
  }
} finally { if (gateway) gateway.kill(); await rm(file, { force: true }); }
