#!/usr/bin/env node
// Architecture guard (Phase D / V1-F5). See docs/plans/2026-10-19-architecture-and-file-size-review.md section 6.
// 1) no static import cycles  2) layer reverse-deps (lib, features)
// 3) raw Tauri invoke() outside src/lib/tauri is baselined and may only shrink
// 4) file size: hard cap 800; the 501-800 band is debt that may only shrink
// schema-handler pairing is covered by the Go test internal/mcp/f11_test.go.
// Usage: node scripts/check-architecture.mjs [--update-baseline]

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(here, "..");
const appSrc = path.join(repoRoot, "easyflowhub-app", "src");
const baselinePath = path.join(here, "architecture-baseline.json");
const UPDATE = process.argv.includes("--update-baseline");

const HARD_CAP = 800;
const DEBT_BAND = 500;
const LAYER_RULES = [
  { layer: "lib", forbid: ["features", "components", "pages", "modules"] },
  { layer: "features", forbid: ["components", "pages"] },
];

const isTest = (f) => /\.(test|spec)\.[tj]sx?$/.test(f) || f.includes("__tests__");

function walk(dir, out) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) { if (e.name !== "node_modules") walk(p, out); }
    else if (/\.(ts|tsx)$/.test(e.name)) out.push(p);
  }
  return out;
}

const allFiles = walk(appSrc, []);
const implFiles = allFiles.filter((f) => !isTest(f));
const rel = (f) => path.relative(appSrc, f).split(path.sep).join("/");
const topDir = (r) => (r.includes("/") ? r.split("/")[0] : "");

function resolveImport(fromFile, spec) {
  if (!spec.startsWith(".")) return null;
  const base = path.resolve(path.dirname(fromFile), spec);
  const cands = [base, base + ".ts", base + ".tsx", path.join(base, "index.ts"), path.join(base, "index.tsx")];
  for (const c of cands) {
    if (fs.existsSync(c) && fs.statSync(c).isFile() && /\.(ts|tsx)$/.test(c)) return c;
  }
  return null;
}

const specRes = [
  new RegExp(String.raw`from\s+[\x27"]([^\x27"]+)[\x27"]`, "g"),
  new RegExp(String.raw`import\s*\(\s*[\x27"]([^\x27"]+)[\x27"]\s*\)`, "g"),
  new RegExp(String.raw`import\s+[\x27"]([^\x27"]+)[\x27"]`, "g"),
];
function importSpecifiers(src) {
  const out = new Set();
  for (const re of specRes) {
    let m; while ((m = re.exec(src))) out.add(m[1]);
  }
  return out;
}

const invokeRe = new RegExp(String.raw`\binvoke\s*\(`);
const notInvokeRe = new RegExp(String.raw`\.\s*invoke\s*\(`);

const graph = new Map();
const rawInvoke = {};
for (const f of implFiles) {
  const src = fs.readFileSync(f, "utf8");
  const targets = new Set();
  for (const spec of importSpecifiers(src)) {
    const r = resolveImport(f, spec);
    if (r) targets.add(r);
  }
  graph.set(f, targets);
  const r = rel(f);
  if (!r.startsWith("lib/tauri/")) {
    let count = 0;
    for (const line of src.split(/\r?\n/)) {
      if (invokeRe.test(line) && !notInvokeRe.test(line) && !/^\s*import\b/.test(line)) count++;
    }
    if (count > 0) rawInvoke[r] = count;
  }
}

const cycles = [];
const color = new Map();
const stack = [];
function dfs(u) {
  color.set(u, 1); stack.push(u);
  for (const v of graph.get(u) || []) {
    const c = color.get(v) || 0;
    if (c === 1) cycles.push([...stack.slice(stack.indexOf(v)), v].map(rel).join(" -> "));
    else if (c === 0) dfs(v);
  }
  stack.pop(); color.set(u, 2);
}
for (const f of implFiles) if ((color.get(f) || 0) === 0) dfs(f);

const layerViolations = [];
for (const f of implFiles) {
  const r = rel(f);
  const rule = LAYER_RULES.find((x) => x.layer === topDir(r));
  if (!rule) continue;
  for (const t of graph.get(f) || []) {
    const td = topDir(rel(t));
    if (rule.forbid.includes(td)) layerViolations.push(`${r} imports ${rel(t)} (layer ${topDir(r)} must not import ${td})`);
  }
}

let baseline = { rawInvoke: {}, sizeDebt: {}, over800Exempt: {} };
if (fs.existsSync(baselinePath)) baseline = JSON.parse(fs.readFileSync(baselinePath, "utf8"));

if (UPDATE) {
  const sizeDebt = {};
  for (const f of implFiles) {
    const n = fs.readFileSync(f, "utf8").split(/\r?\n/).length;
    if (n > DEBT_BAND) sizeDebt[rel(f)] = n;
  }
  const next = {
    _note: "Debt ratchet for check-architecture.mjs. rawInvoke: file -> max allowed invoke() outside lib/tauri. sizeDebt: file -> max allowed lines (only-decrease). over800Exempt: files allowed above the hard cap with a reason.",
    rawInvoke: Object.fromEntries(Object.entries(rawInvoke).sort()),
    sizeDebt: Object.fromEntries(Object.entries(sizeDebt).sort((a, b) => b[1] - a[1])),
    over800Exempt: baseline.over800Exempt || {},
  };
  fs.writeFileSync(baselinePath, JSON.stringify(next, null, 2) + "\n");
  console.log("baseline updated:", baselinePath);
  process.exit(0);
}

const invokeViolations = [];
for (const [file, count] of Object.entries(rawInvoke)) {
  const allowed = baseline.rawInvoke[file];
  if (allowed === undefined) invokeViolations.push(`${file}: ${count} raw invoke() but not in baseline`);
  else if (count > allowed) invokeViolations.push(`${file}: ${count} raw invoke() exceeds baseline ${allowed}`);
}
for (const file of Object.keys(baseline.rawInvoke)) {
  if (!(file in rawInvoke)) invokeViolations.push(`${file}: baseline lists raw invoke but none remain - re-snapshot via --update-baseline`);
}

const sizeViolations = [];
for (const f of implFiles) {
  const r = rel(f);
  const n = fs.readFileSync(f, "utf8").split(/\r?\n/).length;
  const exempt = (baseline.over800Exempt || {})[r];
  if (n > HARD_CAP && !exempt) sizeViolations.push(`${r}: ${n} lines > hard cap ${HARD_CAP} (split it or add to over800Exempt)`);
  if (n > DEBT_BAND) {
    const base = baseline.sizeDebt[r];
    if (base === undefined) sizeViolations.push(`${r}: ${n} lines crosses ${DEBT_BAND} but is not in sizeDebt baseline (new code must stay <= ${DEBT_BAND})`);
    else if (n > base) sizeViolations.push(`${r}: ${n} lines grew past baseline ${base} (debt may only shrink)`);
  } else if (baseline.sizeDebt && (r in baseline.sizeDebt)) {
    sizeViolations.push(`${r}: now ${n} lines (<= ${DEBT_BAND}) - lower/remove its sizeDebt entry via --update-baseline`);
  }
}

const sections = [
  ["import cycles", cycles],
  ["layer reverse-dependencies", layerViolations],
  ["raw invoke baseline", invokeViolations],
  ["file-size debt", sizeViolations],
];
let failed = false;
for (const [name, list] of sections) {
  if (list.length) {
    failed = true;
    console.log(`\nX ${name} (${list.length}):`);
    for (const v of list) console.log("  - " + v);
  } else console.log("ok " + name + ": ok");
}
if (failed) {
  console.log("\nArchitecture guard FAILED. Fix the violations above, or re-snapshot a legitimately-shrunk baseline via --update-baseline.");
  process.exit(1);
}
console.log("\nArchitecture guard passed.");
