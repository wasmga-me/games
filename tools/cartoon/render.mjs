// Renders the SVG sprites a game's art/draw.py wrote (see cartoon.py) into
// PNGs in headless Chromium:  node render.mjs JOBS.json OUT_DIR
import { createRequire } from 'module';
import fs from 'fs';
import path from 'path';

const { chromium } = createRequire(import.meta.url)('playwright');
const [jobsFile, out] = process.argv.slice(2);
const jobs = JSON.parse(fs.readFileSync(jobsFile, 'utf8'));
const dir = path.dirname(path.resolve(jobsFile));
fs.mkdirSync(out, { recursive: true });
const browser = await chromium.launch({
  executablePath: fs.existsSync('/opt/pw-browsers/chromium') ? '/opt/pw-browsers/chromium' : undefined,
});
const page = await browser.newPage();
for (const job of jobs) {
  await page.setViewportSize({ width: job.w, height: job.h });
  await page.setContent(`<body style="margin:0">${fs.readFileSync(path.join(dir, job.svg), 'utf8')}</body>`);
  await page.screenshot({ path: path.join(out, job.name + '.png'), omitBackground: true });
}
console.log(`rendered ${jobs.length} into ${out}`);
await browser.close();
