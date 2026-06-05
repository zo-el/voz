import { chromium } from 'playwright';
import { fileURLToPath } from 'url';
import path from 'path';
import fs from 'fs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const M = (f) => 'file://' + path.join(__dirname, 'mockups', f);
const out = (f) => path.join(__dirname, 'out', f);

// panel screens: tight viewport, transparent-ish desktop backdrop, scale 2 for crispness
const panels = [
  'panel-record-idle',
  'panel-recording',
  'panel-transcribing',
  'panel-result',
  'panel-settings-general',
  'panel-settings-engine',
  'panel-history',
];

const browser = await chromium.launch();
const ctx = await browser.newContext({ deviceScaleFactor: 2 });
const page = await ctx.newPage();

for (const name of panels) {
  await page.setViewportSize({ width: 482, height: 640 });
  await page.goto(M(name + '.html'), { waitUntil: 'networkidle' });
  await page.waitForTimeout(450); // let webfonts settle
  // screenshot just the panel element with its shadow + a little margin
  const el = await page.$('.panel');
  const box = await el.boundingBox();
  const pad = 26;
  await page.screenshot({
    path: out(name + '.png'),
    clip: {
      x: Math.max(0, box.x - pad), y: Math.max(0, box.y - pad),
      width: box.width + pad * 2, height: box.height + pad * 2,
    },
  });
  console.log('rendered', name);
}

// wide desktop/tray context shot
await page.setViewportSize({ width: 1120, height: 660 });
await page.goto(M('tray-context.html'), { waitUntil: 'networkidle' });
await page.waitForTimeout(450);
await page.screenshot({ path: out('tray-context.png') });
console.log('rendered tray-context');

// tray icon states strip
await page.setViewportSize({ width: 640, height: 340 });
await page.goto(M('tray-states.html'), { waitUntil: 'networkidle' });
await page.waitForTimeout(350);
await page.screenshot({ path: out('tray-states.png') });
console.log('rendered tray-states');

// a contact sheet: all panels on one image (images inlined as base64 so they load)
const sheet = await ctx.newPage();
await sheet.setViewportSize({ width: 1520, height: 1180 });
const dataUri = (n) => 'data:image/png;base64,' + fs.readFileSync(out(n + '.png')).toString('base64');
await sheet.setContent(`<!doctype html><html><body style="margin:0;background:#060702;
  display:grid;grid-template-columns:repeat(4,1fr);gap:10px;padding:16px;font-family:Inter,sans-serif">
  ${panels.map((n)=>`<div style="display:flex;flex-direction:column;align-items:center;gap:6px">
     <img src="${dataUri(n)}" style="width:100%;border-radius:8px"/>
     <div style="color:#a9b094;font-size:12px">${n.replace('panel-','')}</div></div>`).join('')}
</body></html>`, { waitUntil: 'load' });
await sheet.waitForTimeout(200);
await sheet.screenshot({ path: out('_contact-sheet.png'), fullPage: true });
console.log('rendered contact sheet');

await browser.close();
