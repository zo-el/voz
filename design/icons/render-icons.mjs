import { chromium } from 'playwright';
import { fileURLToPath } from 'url';
import path from 'path';
import fs from 'fs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const out = (f) => path.join(__dirname, f);

const logo = (bg, badge) => `<!doctype html><html><head><style>
  html,body{margin:0;width:512px;height:512px;background:transparent}
  .wrap{width:512px;height:512px;display:grid;place-items:center}
  .sq{width:432px;height:432px;border-radius:104px;${bg};display:grid;place-items:center;position:relative;
      box-shadow:0 18px 50px rgba(0,0,0,.35)}
  svg{width:248px;height:248px}
  .badge{position:absolute;right:34px;bottom:34px;width:120px;height:120px;border-radius:50%;
         border:18px solid #11140d;${badge||'display:none'}}
</style></head><body><div class="wrap"><div class="sq">
  <svg viewBox="0 0 24 24" fill="none"><path d="M12 3v18M7 8v8M17 8v8M3 11v2M21 11v2"
    stroke="#fff" stroke-width="2.2" stroke-linecap="round"/></svg>
  <span class="badge"></span>
</div></div></body></html>`;

const OLIVE = 'background:linear-gradient(135deg,#8a9a4e,#b3c267)';
const REC = 'background:#d9614a';
const PROC = 'background:#b3c267';

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 512, height: 512 }, deviceScaleFactor: 1 });

async function shot(html, file) {
  await page.setContent(html, { waitUntil: 'load' });
  await page.waitForTimeout(120);
  await page.screenshot({ path: out(file), omitBackground: true });
  console.log('icon', file);
}

await shot(logo(OLIVE), 'icon-src.png');                 // app icon source (512)
await shot(logo(OLIVE), 'tray-idle.png');                // tray idle
await shot(logo(OLIVE, REC), 'tray-rec.png');            // tray recording (red dot)
await shot(logo(OLIVE, PROC), 'tray-proc.png');          // tray processing (olive dot)

await browser.close();
fs.writeFileSync(out('.gitkeep'), '');
