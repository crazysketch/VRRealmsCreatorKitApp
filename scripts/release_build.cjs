// Builds a release: the installer and the exe, signed for the updater, with no trace of the build machine in them.
//   node scripts/release_build.cjs [notes file]
// The updater's private key is read from TAURI_SIGNING_PRIVATE_KEY, else from ~/.tauri/vrrealms-creatorkit-app.key.
// Then it checks the built exe for the build machine's folders and user name, and writes latest.json
// (scripts/updater_manifest.cjs, which also checks the installer's signature against the app's key).
const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');

const root = path.join(__dirname, '..');
const conf = JSON.parse(fs.readFileSync(path.join(root, 'src-tauri', 'tauri.conf.json'), 'utf8'));
const version = conf.version;

const key = process.env.TAURI_SIGNING_PRIVATE_KEY || path.join(os.homedir(), '.tauri', 'vrrealms-creatorkit-app.key');
if (!fs.existsSync(key)) {
  console.error(`No updater key at ${key}. Set TAURI_SIGNING_PRIVATE_KEY to the key file.`);
  process.exit(2);
}

// Error messages of Rust code carry the path of the file they are in. Map every absolute path of this machine
// to a neutral name, most general first: rustc applies the last mapping that matches.
const home = os.homedir();
const cargoHome = process.env.CARGO_HOME || path.join(home, '.cargo');
const rustupHome = process.env.RUSTUP_HOME || path.join(home, '.rustup');
const maps = [
  [home, '~'],
  [rustupHome, 'rustup'],
  [path.join(cargoHome, 'registry', 'src'), 'crates'],
  [path.join(cargoHome, 'git', 'checkouts'), 'git'],
  [root, 'app'],
];
const flags = maps.map(([from, to]) => `--remap-path-prefix=${from}=${to}`);
const env = {
  ...process.env,
  TAURI_SIGNING_PRIVATE_KEY: key,
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD: process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD || '',
  // Separated by 0x1f so a folder name with a space stays one flag.
  CARGO_ENCODED_RUSTFLAGS: flags.join('\x1f'),
};
delete env.RUSTFLAGS;

const run = (cmd, args) => {
  const r = spawnSync(cmd, args, { cwd: root, env, stdio: 'inherit', shell: process.platform === 'win32' });
  if (r.status !== 0) process.exit(r.status || 1);
};
run('npx', ['tauri', 'build']);

// The exe must not name this machine's folders.
const exe = path.join(root, 'src-tauri', 'target', 'release', 'VRRealmsCreatorKit.exe');
const text = fs.readFileSync(exe).toString('latin1');
const user = path.basename(home);
const needles = [home, home.replace(/\\/g, '/'), `\\Users\\${user}\\`, root].map((n) => n.toLowerCase());
const lower = text.toLowerCase();
const found = needles.filter((n) => lower.includes(n));
if (found.length) {
  console.error(`The exe still names this machine's folders: ${found.join(', ')}`);
  process.exit(3);
}
console.log("the exe names none of this machine's folders");

run('node', [path.join('scripts', 'updater_manifest.cjs'), version, ...process.argv.slice(2)]);
