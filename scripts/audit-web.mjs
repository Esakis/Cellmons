import { spawn } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { createServer } from 'node:net';
import { fileURLToPath } from 'node:url';
import lighthouse from 'lighthouse';

const require = createRequire(new URL('../e2e/package.json', import.meta.url));
const { chromium } = require('@playwright/test');
const root = fileURLToPath(new URL('../', import.meta.url));
const server = spawn(
  process.execPath,
  [
    fileURLToPath(new URL('../web/node_modules/vite/bin/vite.js', import.meta.url)),
    'preview',
    '--host',
    '127.0.0.1',
    '--port',
    '4175',
  ],
  { cwd: fileURLToPath(new URL('../web/', import.meta.url)), windowsHide: true, stdio: 'ignore' },
);
let browser;
try {
  const url = 'http://127.0.0.1:4175/';
  let ready = false;
  for (let attempt = 0; attempt < 50; attempt++) {
    if (server.exitCode !== null) throw new Error('Preview server failed to start on port 4175');
    try {
      if ((await fetch(url)).ok) {
        ready = true;
        break;
      }
    } catch {
      /* Wait for Vite. */
    }
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  if (!ready) throw new Error('Preview server did not start');
  const socket = createServer();
  await new Promise((resolve) => socket.listen(0, '127.0.0.1', resolve));
  const address = socket.address();
  if (!address || typeof address === 'string') throw new Error('No debugging port');
  const port = address.port;
  await new Promise((resolve) => socket.close(resolve));
  browser = await chromium.launch({ args: [`--remote-debugging-port=${port}`] });
  const result = await lighthouse(url, {
    port,
    logLevel: 'error',
    onlyCategories: ['performance', 'accessibility', 'best-practices', 'seo'],
    output: 'html',
  });
  if (!result) throw new Error('Lighthouse returned no report');
  await mkdir(`${root}/target/lighthouse`, { recursive: true });
  await writeFile(`${root}/target/lighthouse/report.html`, result.report);
  await writeFile(`${root}/target/lighthouse/report.json`, JSON.stringify(result.lhr, null, 2));
  for (const [key, category] of Object.entries(result.lhr.categories)) {
    console.log(`${key}: ${Math.round((category.score ?? 0) * 100)}`);
    if ((category.score ?? 0) < 0.95) process.exitCode = 1;
  }
} finally {
  await browser?.close();
  server.kill();
}
