// Renders 3D models into PNGs in headless Chromium (three.js, see README.md).
//   node render.mjs JOBS.json OUT_DIR    each job to OUT_DIR/<name>.png
//   node render.mjs --bbox|--anims MODEL...   a model's bounds or animations
import { createRequire } from 'module';
import fs from 'fs';
import http from 'http';
import path from 'path';

const { chromium } = createRequire(import.meta.url)('playwright');
const here = path.dirname(new URL(import.meta.url).pathname);
const roots = { three: path.join(here, 'node_modules/three'), models: path.join(here, 'models') };
// a jobs file's own folder, for textures its jobs.py made
if (!process.argv[2].startsWith('--')) roots.jobs = path.dirname(path.resolve(process.argv[2]));
const types = { '.html': 'text/html', '.js': 'text/javascript' };
const server = http.createServer((q, s) => {
  const url = decodeURIComponent(q.url.split('?')[0]).slice(1);
  const [top, ...rest] = url.split('/');
  const file = roots[top] ? path.join(roots[top], ...rest) : path.join(here, url);
  fs.readFile(file, (e, d) => {
    s.writeHead(e ? 404 : 200, { 'content-type': types[path.extname(file)] || 'application/octet-stream' });
    s.end(d);
  });
}).listen(0);
const browser = await chromium.launch({
  executablePath: fs.existsSync('/opt/pw-browsers/chromium') ? '/opt/pw-browsers/chromium' : undefined,
  args: ['--use-gl=angle', '--use-angle=swiftshader', '--enable-unsafe-swiftshader'],
});
const page = await browser.newPage();
page.on('console', m => m.type() === 'error' && console.error('page:', m.text()));
page.on('pageerror', e => console.error('page:', e.message));
await page.goto(`http://localhost:${server.address().port}/index.html`);
await page.waitForFunction(() => window.ready);
const [first, ...rest] = process.argv.slice(2);
if (first.startsWith('--')) {
  const fn = first.slice(2);
  for (const m of rest) console.log(m, JSON.stringify(await page.evaluate(([fn, m]) => window[fn](m), [fn, m])));
} else {
  const jobs = JSON.parse(fs.readFileSync(first, 'utf8'));
  const out = rest[0];
  fs.mkdirSync(out, { recursive: true });
  for (const job of jobs) {
    const url = await page.evaluate(j => window.render(j), job);
    fs.writeFileSync(path.join(out, job.name + '.png'), Buffer.from(url.split(',')[1], 'base64'));
  }
  console.log(`rendered ${jobs.length} into ${out}`);
}
await browser.close();
server.close();
