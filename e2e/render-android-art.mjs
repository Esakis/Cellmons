// Render the existing project SVG mark, without external artwork or fonts.

import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { chromium } from '@playwright/test';

const svg = await readFile('web/public/icon.svg', 'utf8');
const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage({
    viewport: { width: 512, height: 512 },
    deviceScaleFactor: 1,
  });
  await page.setContent(
    `<style>html,body{margin:0;background:#3d3328}svg{display:block;width:100vw;height:100vh}</style>${svg}`,
  );
  await mkdir('mobile/store/assets', { recursive: true });
  await page.screenshot({ path: 'mobile/store/assets/icon-512.png' });
  for (const [density, size] of [
    ['mdpi', 48],
    ['hdpi', 72],
    ['xhdpi', 96],
    ['xxhdpi', 144],
    ['xxxhdpi', 192],
  ]) {
    await page.setViewportSize({ width: size, height: size });
    const dir = `mobile/android/app/src/main/res/mipmap-${density}`;
    await mkdir(dir, { recursive: true });
    const image = await page.screenshot();
    await writeFile(`${dir}/ic_launcher.png`, image);
    await writeFile(`${dir}/ic_launcher_round.png`, image);
  }
  await page.setViewportSize({ width: 1024, height: 500 });
  await page.setContent(
    `<style>body{margin:0;box-sizing:border-box;width:1024px;height:500px;display:flex;align-items:center;gap:50px;padding:60px;background:#17131c;color:#e6c25a;font-family:Georgia,serif;border:8px double #e6c25a}svg{width:240px;height:240px;flex-shrink:0}h1{font-weight:400;font-size:57px;margin:0 0 20px}p{font-size:26px;letter-spacing:4px;margin:0}</style>${svg}<div><h1>Spellgambit</h1><p>ARCANE CLASS</p></div>`,
  );
  await page.screenshot({ path: 'mobile/store/assets/feature-1024x500.png' });
} finally {
  await browser.close();
}
