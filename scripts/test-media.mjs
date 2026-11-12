import { spawn, execFileSync } from 'node:child_process';
import { readFile, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { startFixtures, root } from './dev-fixtures.mjs';
const close = await startFixtures();
const triple = execFileSync('rustc', ['-vV'], { encoding: 'utf8' }).match(/^host: (.+)$/m)[1].trim();
const file = join(root, 'apps/desktop/e2e/playback.json');
const env = { ...process.env, WATCHTOWER_REAL_MEDIA: '1', WATCHTOWER_TEST_SIDECAR: join(root, `apps/desktop/src-tauri/binaries/go2rtc-${triple}${process.platform === 'win32' ? '.exe' : ''}`), WATCHTOWER_TEST_PLAYBACK: file };
env.WATCHTOWER_TEST_MEDIAMTX = join(root, `apps/desktop/src-tauri/binaries/mediamtx-${triple}${process.platform === 'win32' ? '.exe' : ''}`);
const run = (program, args) => new Promise((resolve, reject) => {
  const child = spawn(program, args, { cwd: root, env, stdio: 'inherit', windowsHide: true });
  child.once('error', reject); child.once('exit', code => code === 0 ? resolve() : reject(new Error(`Test process exited with ${code}`)));
});
let gateway;
try {
  await rm(file, { force: true });
  if (!process.env.WATCHTOWER_BROWSER_ONLY) {
    await run('cargo', ['test', '-p', 'watchtower-media', '--lib', '--', '--ignored']);
    await run('cargo', ['test', '-p', 'watchtower-media', '--test', 'sidecar', '--', '--ignored']);
    await run('cargo', ['test', '-p', 'watchtower-media', '--test', 'rtsp_compatibility', '--', '--ignored']);
    await run('cargo', ['test', '-p', 'watchtower-media', '--test', 'engines', '--', '--ignored']);
    await run('cargo', ['test', '-p', 'watchtower-desktop', '--features', 'go2rtc,mediamtx', 'state::tests', '--', '--ignored']);
  }
  await run('cargo', ['build', '-p', 'watchtower-media', '--example', 'fixture']);
  const engines = process.env.WATCHTOWER_TEST_ENGINE ? [process.env.WATCHTOWER_TEST_ENGINE] : ['go2rtc', 'retina', 'mediamtx'];
  for (const engine of engines) {
    env.WATCHTOWER_TEST_ENGINE = engine;
    await rm(file, { force: true });
    gateway = spawn(join(root, `target/debug/examples/fixture${process.platform === 'win32' ? '.exe' : ''}`), [], { cwd: root, env, stdio: 'inherit', windowsHide: true });
    let ready = false;
    for (let i = 0; i < 300; i++) { if (gateway.exitCode !== null) break; try { await readFile(file); ready = true; break; } catch { await new Promise(resolve => setTimeout(resolve, 200)); } }
    if (!ready) throw new Error('Playback test gateway did not start');
    // Use the JS pnpm entry point rather than asking a shell to resolve a Windows .cmd.
    const pnpm = process.env.npm_execpath;
    if (!pnpm) throw new Error('Run this script with pnpm test:media');
    if (process.env.WATCHTOWER_TEST_GREP) await run(process.execPath, [pnpm, '--filter', '@watchtower/desktop', 'exec', 'playwright', 'test', '--grep', process.env.WATCHTOWER_TEST_GREP]);
    else await run(process.execPath, [pnpm, 'test:smoke']);
    gateway.kill(); gateway = undefined;
  }
} finally {
  if (gateway) gateway.kill();
  close(); await rm(file, { force: true });
}
