// Renders the SVGs draw.py wrote into OUT_DIR to PNGs beside them, with
// sharp (which the site's dependencies bring):
//   node render.mjs OUT_DIR
import fs from "fs";
import path from "path";
import sharp from "sharp";

const out = process.argv[2];
const jobs = JSON.parse(fs.readFileSync(path.join(out, "jobs.json")));
for (const { name } of jobs) {
  await sharp(path.join(out, name + ".svg")).png().toFile(path.join(out, name + ".png"));
}
console.log(jobs.length, "sprites rendered");
