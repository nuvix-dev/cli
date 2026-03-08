#!/usr/bin/env node
const path = require('path');
const fs = require('fs');
const { spawn } = require('child_process');

const exe = process.platform === 'win32' ? 'nuvix.exe' : 'nuvix';
const binPath = path.join(__dirname, '..', 'dist', exe);

if (!fs.existsSync(binPath)) {
  console.error('nuvix binary is missing. Reinstall package: npm i -g nuvix-cli');
  process.exit(1);
}

const child = spawn(binPath, process.argv.slice(2), { stdio: 'inherit' });
child.on('exit', (code, signal) => {
  if (signal) process.kill(process.pid, signal);
  else process.exit(code ?? 1);
});
