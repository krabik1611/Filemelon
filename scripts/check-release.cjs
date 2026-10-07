const fs = require('node:fs');
const path = require('node:path');
const root = path.resolve(__dirname, '..');
const json = file => JSON.parse(fs.readFileSync(path.join(root, file), 'utf8').replace(/^\uFEFF/, ''));
const config = json('src-tauri/tauri.conf.json');
const version = config.version;
if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) throw new Error('Invalid release version');
const versions = [json('package.json').version, json('package-lock.json').packages[''].version];
for (const file of ['src-tauri/Cargo.toml', 'src-tauri/filemelon-core/Cargo.toml']) {
  versions.push(fs.readFileSync(path.join(root, file), 'utf8').match(/^version\s*=\s*"([^"]+)"/m)?.[1]);
}
if (versions.some(value => value !== version)) throw new Error('Versions in npm, Cargo and Tauri must match');
const tag = process.argv[2];
if (tag && tag !== `v${version}`) throw new Error(`Tag ${tag} must match v${version}`);
for (const icon of config.bundle.icon) {
  if (!fs.existsSync(path.join(root, 'src-tauri', icon))) throw new Error(`Missing icon: ${icon}`);
}
console.log(`Release configuration verified: ${version}`);
