// Plays every lesson's solution in the real game code (built with the
// selfcheck feature) and reports any that don't reach their goal.
//   node tools/check.mjs [level] [-v]      check lessons
//   node tools/check.mjs try <level> 'cmd' 'cmd'...   run commands on a lesson
import { readFileSync } from "node:fs";
import { execSync } from "node:child_process";

const here = new URL("..", import.meta.url).pathname;
execSync("cargo build --release --target wasm32-unknown-unknown --features selfcheck --target-dir target/check", {
  cwd: here,
  stdio: "inherit",
});
const bytes = readFileSync(here + "target/check/wasm32-unknown-unknown/release/branchwise.wasm");
let x;
const { instance } = await WebAssembly.instantiate(bytes, {
  env: {
    wg_panic: (p, n) => console.log("panic: " + new TextDecoder().decode(new Uint8Array(x.memory.buffer, p, n))),
  },
});
x = instance.exports;
const text = () => {
  const s = new TextDecoder().decode(new Uint8Array(x.memory.buffer, x.bw_check_text(), x.bw_check_len()));
  x.bw_check_clear();
  return s;
};
const args = process.argv.slice(2);
if (args[0] === "try") {
  const enc = new TextEncoder().encode(args.slice(2).join("\n"));
  const p = x.bw_alloc(enc.length);
  new Uint8Array(x.memory.buffer, p, enc.length).set(enc);
  try {
    x.bw_try(+args[1], p, enc.length);
  } finally {
    process.stdout.write(text());
  }
  process.exit(0);
}
const verbose = args.includes("-v") ? 1 : 0;
const only = args.find((a) => /^\d+$/.test(a));
const n = x.bw_check_levels();
let bad = 0;
for (let i = 0; i < n; i++) {
  if (only !== undefined && i !== +only) continue;
  let r;
  try {
    r = x.bw_check(i, verbose);
  } catch (e) {
    r = 1;
    console.log(text() + `crashed: ${e.message}`);
    // a trap leaves the module unusable
    process.exit(1);
  }
  const t = text();
  if (r !== 0 || verbose) console.log(t);
  if (r !== 0) bad++;
}
console.log(bad ? `${bad} failing` : "all pass");
process.exit(bad ? 1 : 0);
