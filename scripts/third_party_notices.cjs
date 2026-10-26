// Writes THIRD-PARTY-NOTICES.md: every Rust crate built into the Windows app and every JavaScript package bundled into its
// window, with its license, its copyright lines, and one full copy of each license text that appears.
//   node scripts/third_party_notices.cjs          (from the repository root; needs cargo and node_modules)
// Run it again whenever a dependency changes, before a release.
const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');

const ROOT = path.resolve(__dirname, '..');
const OUT = path.join(ROOT, 'THIRD-PARTY-NOTICES.md');
const TARGET = 'x86_64-pc-windows-msvc';

const LICENSE_FILE = /^(licen[cs]e|copying|notice|unlicense)([-._].*)?$/i;

function licenseFiles(dir) {
  try {
    return fs.readdirSync(dir).filter((f) => LICENSE_FILE.test(f) && fs.statSync(path.join(dir, f)).isFile()).sort();
  } catch {
    return [];
  }
}

function copyrightLines(dir) {
  const out = new Set();
  for (const f of licenseFiles(dir)) {
    if (/^notice/i.test(f)) continue;
    const text = fs.readFileSync(path.join(dir, f), 'utf8');
    for (const raw of text.split(/\r?\n/)) {
      const line = raw.trim().replace(/\s+/g, ' ');
      // A real holder line (a year, or "Copyright <name>"), not wording from inside a license body such as
      // "copyright notice" or Apache's "(c) You must retain ...".
      const holder =
        /^(copyright|\(c\)|©)/i.test(line) &&
        !/copyright (notice|holder|owner|license|and|statement)s?\b/i.test(line) &&
        !/\byou\b|\bretain\b/i.test(line) &&
        (/\d{4}/.test(line) || /^copyright\s/i.test(line));
      if (holder && line.length < 200) {
        out.add(line);
      }
    }
  }
  return [...out].slice(0, 4);
}

function noticeText(dir) {
  const f = licenseFiles(dir).find((n) => /^notice/i.test(n));
  return f ? fs.readFileSync(path.join(dir, f), 'utf8').trim() : '';
}

// ---- Rust crates linked into the app (normal dependencies only, Windows target) ----------------------------------------
const meta = JSON.parse(
  execFileSync('cargo', ['metadata', '--format-version', '1', '--filter-platform', TARGET], {
    cwd: path.join(ROOT, 'src-tauri'),
    encoding: 'utf8',
    maxBuffer: 256 * 1024 * 1024,
  }),
);
const pkgs = new Map(meta.packages.map((p) => [p.id, p]));
const nodes = new Map(meta.resolve.nodes.map((n) => [n.id, n]));
const rootId = meta.resolve.root;
const reached = new Set();
const stack = [rootId];
while (stack.length) {
  const id = stack.pop();
  if (reached.has(id)) continue;
  reached.add(id);
  for (const d of nodes.get(id).deps) {
    if (d.dep_kinds.some((k) => k.kind === null)) stack.push(d.pkg);
  }
}
reached.delete(rootId);
const crates = [...reached]
  .map((id) => pkgs.get(id))
  .map((p) => ({
    name: p.name,
    version: p.version,
    license: p.license || (p.license_file ? 'see ' + p.license_file : 'not stated'),
    repo: p.repository || p.homepage || '',
    dir: path.dirname(p.manifest_path),
  }))
  .sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version));

// ---- JavaScript packages bundled into the window (production dependencies, followed down) --------------------------------
const rootPkg = JSON.parse(fs.readFileSync(path.join(ROOT, 'package.json'), 'utf8'));
const jsSeen = new Map();
function walkJs(name, fromDir) {
  let dir = fromDir;
  let pkgDir = null;
  while (true) {
    const tryDir = path.join(dir, 'node_modules', name);
    if (fs.existsSync(path.join(tryDir, 'package.json'))) {
      pkgDir = tryDir;
      break;
    }
    const up = path.dirname(dir);
    if (up === dir) break;
    dir = up;
  }
  if (!pkgDir || jsSeen.has(pkgDir)) return;
  const pj = JSON.parse(fs.readFileSync(path.join(pkgDir, 'package.json'), 'utf8'));
  const repo = typeof pj.repository === 'string' ? pj.repository : (pj.repository && pj.repository.url) || pj.homepage || '';
  jsSeen.set(pkgDir, {
    name: pj.name,
    version: pj.version,
    license: typeof pj.license === 'string' ? pj.license : (pj.license && pj.license.type) || 'not stated',
    repo: repo.replace(/^git\+/, '').replace(/\.git$/, ''),
    dir: pkgDir,
  });
  for (const dep of Object.keys(pj.dependencies || {})) walkJs(dep, pkgDir);
}
for (const dep of Object.keys(rootPkg.dependencies || {})) walkJs(dep, ROOT);
const jsPkgs = [...jsSeen.values()].sort((a, b) => a.name.localeCompare(b.name));

// ---- one full text of every license that appears ---------------------------------------------------------------------------
const MIT_TEXT = `Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated
documentation files (the "Software"), to deal in the Software without restriction, including without limitation the
rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit
persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the
Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE
WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.`;

const all = [...crates, ...jsPkgs];
const ids = new Set();
for (const p of all) {
  for (const id of p.license.split(/\s+OR\s+|\s+AND\s+|\/|\(|\)/i).map((s) => s.trim()).filter(Boolean)) ids.add(id);
}

// Where a license's own text is found: a file named after it, or the only license file of a package under that license alone.
const FILE_HINTS = {
  'Apache-2.0': /apache/i,
  MIT: /mit/i,
  'Unicode-3.0': /unicode/i,
  'BSD-3-Clause': /bsd/i,
  Zlib: /zlib/i,
  ISC: /isc/i,
  'MPL-2.0': /mpl/i,
  Unlicense: /unlicense/i,
  '0BSD': /0bsd/i,
  'CC0-1.0': /cc0/i,
  'MIT-0': /mit-0/i,
  'BSL-1.0': /bsl|boost/i,
  'CDLA-Permissive-2.0': /cdla/i,
};
function findText(id) {
  if (id === 'MIT') return MIT_TEXT;
  const hint = FILE_HINTS[id];
  for (const p of all) {
    if (!new RegExp('(^|[^-\\w])' + id.replace(/[.+]/g, '\\$&') + '($|[^-\\w])').test(p.license)) continue;
    const files = licenseFiles(p.dir).filter((f) => !/^notice/i.test(f));
    const named = hint && files.find((f) => hint.test(f));
    const only = p.license.trim() === id && files.length === 1 ? files[0] : null;
    const pick = named || only;
    if (pick) {
      const text = fs.readFileSync(path.join(p.dir, pick), 'utf8').trim();
      if (text.length > 200) return text;
    }
  }
  return null;
}

// ---- write -----------------------------------------------------------------------------------------------------------------
const L = [];
L.push('# Third-party notices');
L.push('');
L.push('VR Realms Creator Kit (the app) is built with the open-source software listed below. Each part stays under its own');
L.push('license, and nothing in this repository\'s LICENSE.md limits your rights under those licenses. Where a package offers a');
L.push('choice of licenses ("A OR B"), any one of them may be used.');
L.push('');
L.push('The app also downloads programs made by others when you ask it to: the Epic Games Launcher (Epic Games, under its own');
L.push('terms) and SteamCMD (Valve, under the Steam Subscriber Agreement). Neither is part of this repository or of the app.');
L.push('');
L.push(`This file is generated by \`scripts/third_party_notices.cjs\` (${crates.length} Rust crates, ${jsPkgs.length} JavaScript packages).`);
L.push('');
function section(title, list) {
  L.push('## ' + title);
  L.push('');
  for (const p of list) {
    L.push(`### ${p.name} ${p.version}`);
    L.push('');
    // Two trailing spaces keep each line on its own line in Markdown.
    L.push(`License: ${p.license}  `);
    if (p.repo) L.push(`Source: ${p.repo}  `);
    for (const c of copyrightLines(p.dir)) L.push(`${c}  `);
    const notice = noticeText(p.dir);
    if (notice) {
      L.push('');
      L.push('```text');
      L.push(notice);
      L.push('```');
    }
    L.push('');
  }
}
section('Rust crates', crates);
section('JavaScript packages', jsPkgs);
L.push('## License texts');
L.push('');
const missing = [];
for (const id of [...ids].sort()) {
  const text = findText(id);
  L.push('### ' + id);
  L.push('');
  if (text) {
    L.push('```text');
    L.push(text);
    L.push('```');
  } else {
    missing.push(id);
    L.push(`The full text is in the source of each package listed under ${id}, and at https://spdx.org/licenses/${id}.html`);
  }
  L.push('');
}
fs.writeFileSync(OUT, L.join('\n').replace(/\r\n/g, '\n'));
console.log(`wrote ${path.relative(ROOT, OUT)}: ${crates.length} crates, ${jsPkgs.length} JS packages, ${ids.size} licenses` +
  (missing.length ? `; no local text for: ${missing.join(', ')}` : ''));
