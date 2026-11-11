import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { chmod, copyFile, mkdtemp, readFile, readdir, rm, writeFile, mkdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const manifest = JSON.parse(await readFile(join(root, 'scripts/go2rtc-manifest.json'), 'utf8'));
const host = execFileSync('rustc', ['-vV'], { encoding: 'utf8' }).match(/^host: (.+)$/m)?.[1].trim();
const target = process.argv[2] ?? host;
const asset = manifest.targets[target];
if (!asset) throw new Error(`Unsupported target: ${target}`);
const [name, expected] = asset;
const temporary = await mkdtemp(join(tmpdir(), 'watchtower-sidecar-'));
try {
  const response = await fetch(`https://github.com/AlexxIT/go2rtc/releases/download/v${manifest.version}/${name}`, { signal: AbortSignal.timeout(120000) });
  if (!response.ok) throw new Error(`Download failed: HTTP ${response.status}`);
  const bytes = Buffer.from(await response.arrayBuffer());
  if (createHash('sha256').update(bytes).digest('hex') !== expected) throw new Error('Sidecar checksum mismatch');
  let source = join(temporary, name);
  await writeFile(source, bytes);
  if (name.endsWith('.zip')) {
    const extracted = join(temporary, 'extracted');
    if (process.platform === 'win32') {
      execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', '& { param($archive, $destination) Expand-Archive -LiteralPath $archive -DestinationPath $destination }', source, extracted]);
    } else {
      execFileSync('unzip', ['-q', source, '-d', extracted]);
    }
    const entries = await readdir(extracted, { recursive: true });
    const binary = entries.find(entry => /(^|[/\\])go2rtc(?:\.exe)?$/.test(entry));
    if (!binary) throw new Error('Release archive has no go2rtc executable');
    source = join(extracted, binary);
  }
  const directory = join(root, 'apps/desktop/src-tauri/binaries');
  await mkdir(directory, { recursive: true });
  const destination = join(directory, `go2rtc-${target}${target.includes('windows') ? '.exe' : ''}`);
  await copyFile(source, destination);
  if (!target.includes('windows')) await chmod(destination, 0o755);
  console.log(`Verified go2rtc ${manifest.version} for ${target}`);
} finally {
  if (dirname(resolve(temporary)) !== resolve(tmpdir()) || !temporary.startsWith(join(tmpdir(), 'watchtower-sidecar-'))) throw new Error('Unexpected temporary directory');
  await rm(temporary, { recursive: true, force: true });
}
