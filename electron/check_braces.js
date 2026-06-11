const fs = require('fs');
const text = fs.readFileSync('renderer/src/components/TerminalManager.tsx', 'utf8');
const lines = text.split('\n');
let depth = 0;
let inString = false;
let stringChar = '';
let escape = false;
for (let i = 54; i < 635; i++) {
  const line = lines[i];
  for (const ch of line) {
    if (escape) { escape = false; continue; }
    if (ch === '\\' && inString) { escape = true; continue; }
    if (inString) {
      if (ch === stringChar) inString = false;
      continue;
    }
    if (ch === '"' || ch === "'" || ch === '`') {
      stringChar = ch;
      inString = true;
      continue;
    }
    if (ch === '{') depth++;
    if (ch === '}') depth--;
  }
  if (depth < 0) {
    console.log('Negative depth at line', i + 1, 'depth', depth);
    break;
  }
}
console.log('Final depth at line 635:', depth);
