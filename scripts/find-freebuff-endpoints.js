// Dump every occurrence of the identifier CH$ in the binary with context,
// so we can determine what it resolves to (declaration may be far away or
// use a different minified local name per chunk).

const fs = require('fs');
const os = require('os');
const path = require('path');

const exePath = path.join(os.homedir(), '.config', 'manicode', 'freebuff.exe');
const buffer = fs.readFileSync(exePath);
const str = buffer.toString('utf8');

const identifier = 'CH$';
let pos = 0;
let count = 0;

while (count < 15) {
  const idx = str.indexOf(identifier, pos);
  if (idx === -1) break;
  const before = idx > 0 ? str[idx - 1] : '';
  if (!/[A-Za-z0-9_$]/.test(before)) {
    const snippet = str.slice(Math.max(0, idx - 60), idx + 120);
    console.log('@' + idx + ': ' + snippet.replace(/[\x00-\x1f]/g, ' '));
    count += 1;
  }
  pos = idx + identifier.length;
}
console.log('Total shown:', count);
