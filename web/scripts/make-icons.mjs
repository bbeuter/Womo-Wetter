// Rendert die PNG-Icons aus einem SVG per Playwright (Chromium muss vorhanden sein).
import { createRequire } from 'node:module';
// playwright ist global vorinstalliert (NODE_PATH=$(npm root -g)) oder lokal
const { chromium } = createRequire(import.meta.url)('playwright');
import fs from 'node:fs';
const sun = (bg, scale) => `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512"><rect width="512" height="512" fill="${bg}"/>
<g transform="translate(256 256) scale(${scale}) translate(-256 -256)" fill="none" stroke="#fff" stroke-width="30" stroke-linecap="round">
<circle cx="256" cy="256" r="84" fill="#fff"/><path d="M256 72v52M256 388v52M72 256h52M388 256h52M126 126l37 37M349 349l37 37M126 386l37-37M349 163l37-37"/></g></svg>`;
const out = new URL('../public/icons/', import.meta.url);
const jobs = [['apple-touch-icon.png', 180, 0.92], ['icon-192.png', 192, 0.92], ['icon-512.png', 512, 0.92], ['icon-maskable-512.png', 512, 0.72]];
const browser = await chromium.launch();
for (const [name, size, scale] of jobs) {
  const page = await browser.newPage({ viewport: { width: size, height: size } });
  await page.setContent(`<style>html,body{margin:0}svg{display:block;width:${size}px;height:${size}px}</style>${sun('#F5B400', scale)}`);
  fs.writeFileSync(new URL(name, out), await page.screenshot({ type: 'png' }));
  await page.close();
}
await browser.close();
console.log('Icons geschrieben');
