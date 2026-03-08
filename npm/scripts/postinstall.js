const fs = require('fs');
const path = require('path');
const os = require('os');
const https = require('https');
const { execFileSync } = require('child_process');

const pkg = require('../package.json');
const version = pkg.version;
const repo = process.env.NUVIX_CLI_REPO || 'nuvix-dev/cli';
const tag = `v${version}`;

const map = {
  'darwin-x64': { target: 'x86_64-apple-darwin', ext: 'tar.gz', bin: 'nuvix' },
  'darwin-arm64': { target: 'aarch64-apple-darwin', ext: 'tar.gz', bin: 'nuvix' },
  'linux-x64': { target: 'x86_64-unknown-linux-musl', ext: 'tar.gz', bin: 'nuvix' },
  'linux-arm64': { target: 'aarch64-unknown-linux-musl', ext: 'tar.gz', bin: 'nuvix' },
  'win32-x64': { target: 'x86_64-pc-windows-msvc', ext: 'zip', bin: 'nuvix.exe' }
};

const key = `${process.platform}-${process.arch}`;
const spec = map[key];
if (!spec) {
  console.error(`Unsupported platform: ${key}`);
  process.exit(1);
}

const file = `nuvix-${spec.target}.${spec.ext}`;
const url = `https://github.com/${repo}/releases/download/${tag}/${file}`;
const distDir = path.join(__dirname, '..', 'dist');
const archivePath = path.join(os.tmpdir(), file);

fs.mkdirSync(distDir, { recursive: true });

function download(src, dest) {
  return new Promise((resolve, reject) => {
    const req = https.get(src, (res) => {
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        return resolve(download(res.headers.location, dest));
      }
      if (res.statusCode !== 200) {
        reject(new Error(`Download failed (${res.statusCode}): ${src}`));
        return;
      }
      const out = fs.createWriteStream(dest);
      res.pipe(out);
      out.on('finish', () => out.close(resolve));
      out.on('error', reject);
    });
    req.on('error', reject);
  });
}

(async () => {
  try {
    await download(url, archivePath);

    if (spec.ext === 'tar.gz') {
      execFileSync('tar', ['-xzf', archivePath, '-C', distDir], { stdio: 'inherit' });
    } else {
      if (process.platform === 'win32') {
        execFileSync('powershell', ['-NoProfile', '-Command', `Expand-Archive -Path "${archivePath}" -DestinationPath "${distDir}" -Force`], { stdio: 'inherit' });
      } else {
        execFileSync('unzip', ['-o', archivePath, '-d', distDir], { stdio: 'inherit' });
      }
    }

    const binPath = path.join(distDir, spec.bin);
    if (!fs.existsSync(binPath)) {
      throw new Error(`Extracted binary not found: ${binPath}`);
    }

    if (process.platform !== 'win32') {
      fs.chmodSync(binPath, 0o755);
    }

    console.log(`Installed nuvix ${version} (${spec.target})`);
  } catch (err) {
    console.error(`Failed to install nuvix binary: ${err.message}`);
    process.exit(1);
  } finally {
    if (fs.existsSync(archivePath)) fs.rmSync(archivePath, { force: true });
  }
})();
