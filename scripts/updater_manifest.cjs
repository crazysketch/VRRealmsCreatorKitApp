// Writes latest.json for a release: what installed copies of the app read to update themselves.
// It first checks the installer's signature against the public key built into the app (tauri.conf.json), the same check
// the app makes before it installs anything, so a release never names an installer the app would refuse.
//   node scripts/updater_manifest.cjs <version> [notes file]
// Reads   src-tauri/target/release/bundle/nsis/VR Realms Creator Kit_<version>_x64-setup.exe (+ .sig)
// Writes  src-tauri/target/release/bundle/nsis/latest.json (upload it with the release, beside VRRealmsCreatorKitApp-Setup.exe)
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const version = process.argv[2];
if (!/^\d+\.\d+\.\d+$/.test(version || '')) {
  console.error('usage: node scripts/updater_manifest.cjs <version> [notes file]');
  process.exit(2);
}
const root = path.join(__dirname, '..');
const dir = path.join(root, 'src-tauri', 'target', 'release', 'bundle', 'nsis');
const setup = path.join(dir, `VR Realms Creator Kit_${version}_x64-setup.exe`);
const sigText = fs.readFileSync(setup + '.sig', 'utf8').trim();
const conf = JSON.parse(fs.readFileSync(path.join(root, 'src-tauri', 'tauri.conf.json'), 'utf8'));
if (conf.version !== version) throw new Error(`tauri.conf.json says ${conf.version}, not ${version}`);

// Minisign, as the updater uses it: the key and the signature files are base64 of minisign's text files.
const keyLines = Buffer.from(conf.plugins.updater.pubkey, 'base64').toString('utf8').trim().split('\n');
const key = Buffer.from(keyLines[1], 'base64'); // 'Ed' + 8 byte key id + 32 byte public key
const sigLines = Buffer.from(sigText, 'base64').toString('utf8').trim().split('\n');
const sig = Buffer.from(sigLines[1], 'base64'); // 'ED' (prehashed) + 8 byte key id + 64 byte signature
if (key.subarray(0, 2).toString() !== 'Ed') throw new Error('the public key is not an Ed25519 minisign key');
if (sig.subarray(0, 2).toString() !== 'ED') throw new Error('the signature is not a prehashed minisign signature');
if (!key.subarray(2, 10).equals(sig.subarray(2, 10))) throw new Error('the installer was signed with ANOTHER key than the app trusts');
const publicKey = crypto.createPublicKey({
  key: Buffer.concat([Buffer.from('302a300506032b6570032100', 'hex'), key.subarray(10, 42)]),
  format: 'der',
  type: 'spki',
});
const digest = crypto.createHash('blake2b512').update(fs.readFileSync(setup)).digest();
if (!crypto.verify(null, digest, publicKey, sig.subarray(10, 74))) throw new Error('the installer does NOT match its signature');
// The trusted comment is signed too (signature + comment).
const trusted = sigLines[2].replace(/^trusted comment: /, '');
const global = Buffer.from(sigLines[3], 'base64');
if (!crypto.verify(null, Buffer.concat([sig.subarray(10, 74), Buffer.from(trusted, 'utf8')]), publicKey, global)) {
  throw new Error("the signature's trusted comment does not check out");
}

const notes = process.argv[3] ? fs.readFileSync(process.argv[3], 'utf8').trim() : `VR Realms Creator Kit ${version}`;
const manifest = {
  version,
  notes,
  pub_date: new Date().toISOString().replace(/\.\d+Z$/, 'Z'),
  platforms: {
    'windows-x86_64': {
      signature: sigText,
      url: `https://github.com/crazysketch/VRRealmsCreatorKitApp/releases/download/v${version}/VRRealmsCreatorKitApp-Setup.exe`,
    },
  },
};
fs.writeFileSync(path.join(dir, 'latest.json'), JSON.stringify(manifest, null, 2) + '\n');
console.log(`signature checks out against the app's key; latest.json written for ${version}`);
