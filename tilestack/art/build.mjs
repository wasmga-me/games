// Renders the tile pictures (art/riichi/*.svg by FluffyStuff, CC0, and our
// own art/extra/*.svg) into src/art.bin, which the game embeds:
//
//   "TART", u16 faces, u16 width, u16 height, then for each face:
//   u8 colours, colours x u32 (premultiplied 0xAABBGGRR, as wg draws),
//   u32 length, then the colour index of every pixel, row by row, packed:
//   a byte n < 128 is followed by n + 1 indices, a byte n >= 128 by one
//   index that repeats n - 126 times.
//
// Run: node tilestack/art/build.mjs (needs sharp, which the site's
// dependencies bring; elsewhere npm install sharp).
import sharp from "sharp";
import { readFileSync, writeFileSync } from "node:fs";

const DIR = new URL(".", import.meta.url).pathname;
const OUT = new URL("../src/art.bin", import.meta.url).pathname;
/** Pixels across a picture; the game scales it down to the tile. */
const WIDTH = 150;
const HEIGHT = Math.round((WIDTH * 4) / 3);
/** Colours a picture keeps; with more, sharp keeps 256 (an 8-bit palette). */
const COLOURS = 16;

// in the order of the faces in board.rs
const FACES = [
  ...[1, 2, 3, 4, 5, 6, 7, 8, 9].map((n) => `riichi/Pin${n}`),
  ...[1, 2, 3, 4, 5, 6, 7, 8, 9].map((n) => `riichi/Sou${n}`),
  ...[1, 2, 3, 4, 5, 6, 7, 8, 9].map((n) => `riichi/Man${n}`),
  "riichi/Ton", "riichi/Nan", "riichi/Shaa", "riichi/Pei",
  "riichi/Chun", "riichi/Hatsu", "extra/Haku",
  "extra/Plum", "extra/Orchid", "extra/Chrysanthemum", "extra/Lotus",
  "extra/Spring", "extra/Summer", "extra/Autumn", "extra/Winter",
];

async function face(name) {
  const svg = readFileSync(`${DIR}${name}.svg`);
  // every picture is drawn on a 300 x 400 canvas
  const png = await sharp(svg, { density: (72 * WIDTH) / 300 })
    .resize(WIDTH, HEIGHT, { fit: "fill" })
    .png({ palette: true, colours: COLOURS, dither: 0, effort: 10 })
    .toBuffer();
  const { data } = await sharp(png).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
  const colours = new Map();
  const index = new Uint8Array(WIDTH * HEIGHT);
  for (let i = 0; i < index.length; i++) {
    let [r, g, b, a] = data.subarray(i * 4, i * 4 + 4);
    if (a < 4) r = g = b = a = 0;
    const pm = (c) => Math.round((c * a) / 255);
    const key = (((a << 24) | (pm(b) << 16) | (pm(g) << 8) | pm(r)) >>> 0);
    if (!colours.has(key)) colours.set(key, colours.size);
    index[i] = colours.get(key);
  }
  if (colours.size > 255) throw new Error(`${name}: ${colours.size} colours`);
  const packed = [];
  for (let i = 0; i < index.length; ) {
    let run = 1;
    while (i + run < index.length && index[i + run] === index[i] && run < 129) run++;
    if (run >= 2) {
      packed.push(run + 126, index[i]);
      i += run;
      continue;
    }
    // literals until the next run of 3 or more
    const start = i;
    while (i < index.length && i - start < 128) {
      if (i + 2 < index.length && index[i] === index[i + 1] && index[i] === index[i + 2]) break;
      i++;
    }
    packed.push(i - start - 1, ...index.subarray(start, i));
  }
  const out = Buffer.alloc(1 + colours.size * 4 + 4 + packed.length);
  let o = out.writeUInt8(colours.size, 0);
  for (const c of colours.keys()) o = out.writeUInt32LE(c, o);
  o = out.writeUInt32LE(packed.length, o);
  Buffer.from(packed).copy(out, o);
  return out;
}

const head = Buffer.alloc(10);
head.write("TART", 0);
head.writeUInt16LE(FACES.length, 4);
head.writeUInt16LE(WIDTH, 6);
head.writeUInt16LE(HEIGHT, 8);
const parts = [head];
// one at a time: sharp quantizes wrongly with several at once
for (const name of FACES) parts.push(await face(name));
writeFileSync(OUT, Buffer.concat(parts));
console.log(`${OUT}: ${FACES.length} faces at ${WIDTH}x${HEIGHT}, ${Buffer.concat(parts).length} bytes`);
