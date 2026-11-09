import { execFileSync } from 'node:child_process';
import { readFile, readdir, writeFile, mkdir } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const metadata = JSON.parse(execFileSync('cargo', ['metadata', '--format-version', '1', '--locked'], { cwd: root, encoding: 'utf8', maxBuffer: 30 * 1024 * 1024 }));
const items = [];
async function licenses(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const paths = entries.filter(entry => entry.isFile() && /^(licen[sc]e|copying|notice)([-_.].*)?$/i.test(entry.name)).map(entry => join(directory, entry.name));
  const text = [];
  for (const path of paths) { const bytes = await readFile(path); if (bytes.length < 100000) text.push(bytes.toString('utf8')); }
  return text.join('\n\n');
}
for (const pkg of metadata.packages.filter(pkg => pkg.source)) {
  const directory = dirname(pkg.manifest_path);
  items.push({ name: pkg.name, version: pkg.version, license: pkg.license ?? 'See source', source: `https://crates.io/crates/${pkg.name}/${pkg.version}`, text: await licenses(directory) });
}
const seen = new Set();
for (const entry of await readdir(join(root, 'node_modules/.pnpm'), { withFileTypes: true })) {
  if (!entry.isDirectory()) continue;
  const base = join(root, 'node_modules/.pnpm', entry.name, 'node_modules');
  let names; try { names = await readdir(base); } catch { continue; }
  const directories = [];
  for (const name of names) {
    if (name.startsWith('@')) { for (const child of await readdir(join(base, name))) directories.push(join(base, name, child)); }
    else if (!name.startsWith('.')) directories.push(join(base, name));
  }
  for (const directory of directories) {
    try {
      const pkg = JSON.parse(await readFile(join(directory, 'package.json'), 'utf8'));
      const key = `${pkg.name}@${pkg.version}`; if (seen.has(key)) continue; seen.add(key);
      items.push({ name: pkg.name, version: pkg.version, license: typeof pkg.license === 'string' ? pkg.license : 'See source', source: `https://www.npmjs.com/package/${pkg.name}/v/${pkg.version}`, text: await licenses(directory) });
    } catch { /* Optional platform packages may not be installed. */ }
  }
}
items.sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version));
const header = '# Third-party notices\n\nGenerated from Cargo.lock and installed pnpm packages. This includes build, test and alternate-platform dependencies. Original license texts below are unmodified; source links identify each version. The application itself is MIT licensed. System webviews are provided by the operating system.\n\nBundled media engine and player: go2rtc v1.9.15, MIT, https://github.com/AlexxIT/go2rtc/tree/v1.9.15 . Its complete license is included alongside this file as go2rtc.txt.\n\n';
await writeFile(join(root, 'THIRD_PARTY_NOTICES.md'), header + items.map(item => `## ${item.name} ${item.version}\n\nLicense: ${item.license}\n\nSource: ${item.source}\n\n${item.text || 'License text is available in the source distribution linked above.'}\n`).join('\n'));
await mkdir(join(root, 'docs'), { recursive: true });
await writeFile(join(root, 'docs/dependency-licenses.json'), JSON.stringify(items.map(({ name, version, license, source }) => ({ name, version, license, source })), null, 2) + '\n');
console.log(`Generated notices for ${items.length} dependency versions`);
