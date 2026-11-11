import { execFileSync } from 'node:child_process';
import { join } from 'node:path';
import { startFixtures, root } from './dev-fixtures.mjs';
const close = await startFixtures();
try {
  const triple = execFileSync('rustc', ['-vV'], { encoding: 'utf8' }).match(/^host: (.+)$/m)[1].trim();
  const suffix = process.platform === 'win32' ? '.exe' : '';
  const env = { ...process.env, WATCHTOWER_TEST_SIDECAR: join(root, `apps/desktop/src-tauri/binaries/go2rtc-${triple}${suffix}`), WATCHTOWER_TEST_MEDIAMTX: join(root, `apps/desktop/src-tauri/binaries/mediamtx-${triple}${suffix}`) };
  const selection = process.argv.includes('--native-lifecycle') ? ['--lib', 'native::tests'] : ['--test', 'engines'];
  execFileSync('cargo', ['test', '-p', 'watchtower-media', ...selection, '--', '--ignored'], { cwd: root, env, stdio: 'inherit', windowsHide: true });
} finally { close(); }
