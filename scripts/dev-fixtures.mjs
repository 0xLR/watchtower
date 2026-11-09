import { createHash } from 'node:crypto';
import { execFileSync, spawn } from 'node:child_process';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createConnection } from 'node:net';
export const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const pins = {
  'win32-x64': ['windows_amd64.zip', 'b2f5246b07cb72961df03b8d5e8cad43dc657bcf3f49a87bb031bdf22721af74'],
  'linux-x64': ['linux_amd64.tar.gz', '0a2ea7eba96023631efa1bea7804823a9145b043b6646a8a05798d4ac24ed635'],
  'darwin-x64': ['darwin_amd64.tar.gz', '8a8416ee2ff5d17d9366498b6d88b76ab6d3afcda50b6d7321ccddb349bdbaf2'],
  'darwin-arm64': ['darwin_arm64.tar.gz', 'bc1a0762fd18ffb413bcd81dcb9a072ac159fbd1f27e79ac6f14598aaac5ed3f'],
};
const wait = ms => new Promise(resolve => setTimeout(resolve, ms));
function portOpen() { return new Promise(resolve => { const socket = createConnection({ host: '127.0.0.1', port: 8554 }); socket.setTimeout(300); socket.once('connect', () => { socket.destroy(); resolve(true); }); socket.once('error', () => resolve(false)); socket.once('timeout', () => { socket.destroy(); resolve(false); }); }); }
export async function startFixtures() {
  if (await portOpen()) throw new Error('Port 8554 is already in use. Stop the earlier development fixture first.');
  const pin = pins[`${process.platform}-${process.arch}`]; if (!pin) throw new Error('Unsupported fixture platform');
  const directory = join(root, '.tools/mediamtx'); await mkdir(directory, { recursive: true });
  const name = `mediamtx_v1.21.3_${pin[0]}`, archive = join(root, '.tools', name);
  let bytes; try { bytes = await readFile(archive); } catch { /* download below */ }
  if (!bytes || createHash('sha256').update(bytes).digest('hex') !== pin[1]) {
    const response = await fetch(`https://github.com/bluenviron/mediamtx/releases/download/v1.21.3/${name}`, { signal: AbortSignal.timeout(120000) });
    if (!response.ok) throw new Error(`Fixture download failed: ${response.status}`);
    bytes = Buffer.from(await response.arrayBuffer());
    if (createHash('sha256').update(bytes).digest('hex') !== pin[1]) throw new Error('Fixture checksum mismatch');
    await writeFile(archive, bytes);
  }
  if (process.platform === 'win32') execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', '& { param($archive, $destination) Expand-Archive -LiteralPath $archive -DestinationPath $destination -Force }', archive, directory]);
  else execFileSync('tar', ['-xzf', archive, '-C', directory]);
  const config = join(directory, 'fixture.yml');
  await writeFile(config, 'logLevel: error\nrtspAddress: 127.0.0.1:8554\nrtspTransports: [tcp]\nrtmp: no\nhls: no\nwebrtc: no\nsrt: no\npaths:\n  all_others:\n');
  const children = [];
  const launch = (program, args) => {
    const child = spawn(program, args, { stdio: 'ignore', windowsHide: true });
    child.on('error', () => console.error('A synthetic-camera process could not start. Ensure FFmpeg is installed.'));
    children.push(child); return child;
  };
  const close = () => { for (const child of children) child.kill(); };
  try {
    launch(join(directory, `mediamtx${process.platform === 'win32' ? '.exe' : ''}`), [config]);
    let ready = false; for (let i = 0; i < 50; i++) { if (await portOpen()) { ready = true; break; } await wait(100); }
    if (!ready) throw new Error('Synthetic RTSP server did not start');
    for (const [size, path] of [['1280x720', 'watchtower-test'], ['640x360', 'watchtower-sub']]) {
      launch('ffmpeg', ['-hide_banner', '-loglevel', 'error', '-re', '-f', 'lavfi', '-i', `testsrc2=size=${size}:rate=15`, '-an', '-c:v', 'libx264', '-preset', 'ultrafast', '-tune', 'zerolatency', '-pix_fmt', 'yuv420p', '-g', '15', '-f', 'rtsp', '-rtsp_transport', 'tcp', `rtsp://127.0.0.1:8554/${path}`]);
    }
    const verify = async path => {
      for (let attempt = 0; attempt < 15; attempt++) {
        const okay = await new Promise(resolve => {
          const child = spawn('ffprobe', ['-v', 'error', '-rtsp_transport', 'tcp', '-select_streams', 'v:0', '-show_entries', 'stream=width', '-of', 'csv=p=0', `rtsp://127.0.0.1:8554/${path}`], { stdio: 'ignore', windowsHide: true });
          const timer = setTimeout(() => { child.kill(); resolve(false); }, 4000);
          child.once('error', () => { clearTimeout(timer); resolve(false); });
          child.once('exit', code => { clearTimeout(timer); resolve(code === 0); });
        });
        if (okay) return;
        await wait(200);
      }
      throw new Error('Synthetic RTSP video was not ready; check FFmpeg/FFprobe availability');
    };
    await Promise.all(['watchtower-test', 'watchtower-sub'].map(verify));
    if (children.some(child => child.exitCode !== null)) throw new Error('A synthetic camera stopped. Check FFmpeg availability.');
    return close;
  } catch (error) { close(); throw error; }
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const close = await startFixtures(); console.log('Synthetic cameras ready on localhost:8554; press Ctrl+C to stop.');
  process.once('SIGINT', () => { close(); process.exit(); }); process.once('SIGTERM', () => { close(); process.exit(); });
}
