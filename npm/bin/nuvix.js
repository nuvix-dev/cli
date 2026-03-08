#!/usr/bin/env node

const path = require('path')
const fs = require('fs')
const { spawn } = require('child_process')

const exe = process.platform === 'win32' ? 'nuvix.exe' : 'nuvix'
const binPath = path.join(__dirname, '..', 'dist', exe)

if (!fs.existsSync(binPath)) {
  console.error('nuvix binary missing. Reinstall: npm install -g nuvix')
  process.exit(1)
}

const child = spawn(binPath, process.argv.slice(2), {
  stdio: 'inherit'
})

child.on('error', err => {
  console.error(err)
  process.exit(1)
})

child.on('exit', code => {
  process.exit(code ?? 1)
})