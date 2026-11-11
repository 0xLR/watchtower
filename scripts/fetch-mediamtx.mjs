import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { chmod, copyFile, mkdtemp, readFile, readdir, rm, writeFile, mkdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const manifest = JSON.parse(await readFile(join(root, 'scripts/mediamtx-manifest.json'), 'utf8'));
const host = execFileSync('rustc', ['-vV'], { encoding: 'utf8' }).match(/^host: (.+)$/m)?.[1].trim();
const target = process.argv[2] ?? host;
const asset = manifest.targets[target];
if (!asset) throw new Error(`No pinned MediaMTX release for target: ${target}`);
const [suffix, expected] = asset;
const name = `mediamtx_v${manifest.version}_${suffix}`;
const temporary = await mkdtemp(join(tmpdir(), 'watchtower-mediamtx-'));
try {
  let bytes;
  try { bytes = await readFile(join(root, '.tools', name)); } catch { /* Fetch below. */ }
  if (!bytes || createHash('sha256').update(bytes).digest('hex') !== expected) {
    const response = await fetch(`https://github.com/bluenviron/mediamtx/releases/download/v${manifest.version}/${name}`, { signal: AbortSignal.timeout(120000) });
    if (!response.ok) throw new Error(`MediaMTX download failed: HTTP ${response.status}`);
    bytes = Buffer.from(await response.arrayBuffer());
  }
  if (createHash('sha256').update(bytes).digest('hex') !== expected) throw new Error('MediaMTX checksum mismatch');
  const archive = join(temporary, name), extracted = join(temporary, 'extracted');
  await writeFile(archive, bytes); await mkdir(extracted);
  if (name.endsWith('.zip')) {
    if (process.platform === 'win32') execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', '& { param($archive, $destination) Expand-Archive -LiteralPath $archive -DestinationPath $destination }', archive, extracted]);
    else execFileSync('unzip', ['-q', archive, '-d', extracted]);
  } else execFileSync('tar', ['-xzf', archive, '-C', extracted]);
  const entries = await readdir(extracted, { recursive: true });
  const binary = entries.find(entry => /(^|[/\\])mediamtx(?:\.exe)?$/.test(entry));
  if (!binary) throw new Error('Release archive has no MediaMTX executable');
  const directory = join(root, 'apps/desktop/src-tauri/binaries'); await mkdir(directory, { recursive: true });
  const destination = join(directory, `mediamtx-${target}${target.includes('windows') ? '.exe' : ''}`);
  await copyFile(join(extracted, binary), destination);
  if (!target.includes('windows')) await chmod(destination, 0o755);
  const license = entries.find(entry => /(^|[/\\])LICENSE$/.test(entry));
  if (!license) throw new Error('Release archive has no license');
  await mkdir(join(root, 'apps/desktop/src-tauri/licenses'), { recursive: true });
  await copyFile(join(extracted, license), join(root, 'apps/desktop/src-tauri/licenses/mediamtx.txt'));
  console.log(`Verified MediaMTX ${manifest.version} for ${target}`);
} finally {
  if (dirname(resolve(temporary)) !== resolve(tmpdir()) || !temporary.startsWith(join(tmpdir(), 'watchtower-mediamtx-'))) throw new Error('Unexpected temporary directory');
  await rm(temporary, { recursive: true, force: true });
}
