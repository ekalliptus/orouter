// Locate where the Freebuff binary adds runId to chat completion requests.
// Search for "runId" occurrences near "chat/completions" call sites and
// request-body assembly, then print the surrounding context.

const fs = require('fs');
const os = require('os');
const path = require('path');

const exePath = path.join(os.homedir(), '.config', 'manicode', 'freebuff.exe');
const buffer = fs.readFileSync(exePath);
const str = buffer.toString('utf8');

const needle = 'No runId found in request body';
let idx = str.indexOf(needle);
if (idx === -1) {
  console.log('error message not found in binary');
  process.exit(0);
}

// Print context before the validation to see which field is being read
console.log('=== context around "No runId found in request body" ===');
console.log(str.slice(Math.max(0, idx - 1200), idx + 300).replace(/[\x00-\x1f]/g, ' '));
