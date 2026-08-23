#!/usr/bin/env node

'use strict';

const path = require('node:path');
const fs = require('node:fs');
const { spawn } = require('node:child_process');

function targetKey() {
  if (process.platform === 'linux' && process.arch === 'x64') return 'x86_64-unknown-linux-gnu';
  if (process.platform === 'linux' && process.arch === 'arm64') return 'aarch64-unknown-linux-gnu';
  if (process.platform === 'darwin' && process.arch === 'arm64') return 'aarch64-apple-darwin';
  if (process.platform === 'win32' && process.arch === 'x64') return 'x86_64-pc-windows-gnu';
  return null;
}

const target = targetKey();
if (!target) {
  console.error(`@fffonion/yahu does not provide a binary for ${process.platform}/${process.arch}.`);
  process.exit(1);
}

const executable = process.platform === 'win32' ? 'yahu.exe' : 'yahu';
const binary = path.join(__dirname, '..', 'binaries', target, executable);
if (!fs.existsSync(binary)) {
  console.error(`The @fffonion/yahu package is missing its ${target} binary.`);
  process.exit(1);
}

const child = spawn(binary, process.argv.slice(2), {
  stdio: 'inherit',
  windowsHide: false,
});
child.on('error', (error) => {
  console.error(`Failed to start yahu: ${error.message}`);
  process.exit(1);
});
child.on('exit', (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
  } else {
    process.exit(code === null ? 1 : code);
  }
});
