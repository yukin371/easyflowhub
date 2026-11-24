#!/usr/bin/env node
// Native Runtime architecture guard (ADR-0009, plan 2026-11-24-native-runtime-redesign section 5.7).
//
// Rules enforced once native/ exists:
//   1) crate dependency edges must follow the allowed matrix (no cycles)
//   2) platform / UI / storage / network symbols must not leak into the wrong crate
//   3) every .rs file stays <= 800 lines
// Before the native workspace is created the guard prints a skip line and exits 0,
// so the existing CI stays green during Phase 0.
//
// Usage: node scripts/check-native-architecture.mjs

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(here, "..");
const nativeRoot = path.join(repoRoot, "native");

const HARD_CAP = 800;

const ALLOWED_EDGES = {
  app: ["hub-ui", "hub-editor", "hub-win", "hub-core", "hub-bridge"],
  "hub-ui": ["hub-editor", "hub-win", "hub-core"],
  "hub-editor": ["hub-win", "hub-core"],
  "hub-bridge": ["hub-core"],
  "hub-win": [],
  "hub-core": [],
};

const FORBIDDEN_SYMBOLS = {
  app: [/tauri/],
  "hub-core": [/\b(egui|eframe|windows|tauri|reqwest)::/, /^\s*use\s+(egui|eframe|windows|tauri|reqwest)\b/m],
  "hub-ui": [/\b(windows|rusqlite|reqwest|tauri)::/, /^\s*use\s+(windows|rusqlite|reqwest|tauri)\b/m],
  "hub-editor": [/\b(egui|eframe|rusqlite|tauri)::/, /^\s*use\s+(egui|eframe|rusqlite|tauri)\b/m],
  "hub-win": [/\b(egui|eframe|rusqlite|tauri)::/, /^\s*use\s+(egui|eframe|rusqlite|tauri)\b/m],
  "hub-bridge": [/\b(egui|eframe|windows|rusqlite|tauri)::/, /^\s*use\s+(egui|eframe|windows|rusqlite|tauri)\b/m],
};

const normalize = (name) => name.replace(/_/g, "-");

function walk(dir, predicate, out = []) {
  if (!fs.existsSync(dir)) return out;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (entry.name === "target" || entry.name === ".git") continue;
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, predicate, out);
    else if (predicate(full)) out.push(full);
  }
  return out;
}

function parseCargoToml(file) {
  const text = fs.readFileSync(file, "utf8");
  const nameMatch = text.match(/^\s*name\s*=\s*"([^"]+)"/m);
  const dir = path.dirname(file);
  const deps = new Set();
  let section = "";

  for (const raw of text.split(/\r?\n/)) {
    const line = raw.replace(/#.*$/, "").trim();
    if (!line) continue;

    const sec = line.match(/^\[(.+)\]$/);
    if (sec) {
      section = sec[1];
      const tableDep = section.match(/^(?:dev-|build-)?dependencies\.(.+)$/);
      if (tableDep) deps.add(normalize(tableDep[1]));
      continue;
    }

    if (/^(dev-|build-)?dependencies$/.test(section)) {
      const kv = line.match(/^([A-Za-z0-9_-]+)\s*=/);
      if (kv) deps.add(normalize(kv[1]));
    }
  }

  return {
    name: nameMatch ? normalize(nameMatch[1]) : normalize(path.basename(dir)),
    dir,
    deps: [...deps],
  };
}

if (!fs.existsSync(nativeRoot)) {
  console.log("skip native architecture guard: native/ not created yet (Phase 0).");
  process.exit(0);
}

const crates = [];
const crateDirs = [];
const cratesRoot = path.join(nativeRoot, "crates");
if (fs.existsSync(cratesRoot)) {
  for (const entry of fs.readdirSync(cratesRoot, { withFileTypes: true })) {
    if (entry.isDirectory()) crateDirs.push(path.join(cratesRoot, entry.name));
  }
}
const appDir = path.join(nativeRoot, "app");
if (fs.existsSync(appDir)) crateDirs.push(appDir);

for (const dir of crateDirs) {
  const cargo = path.join(dir, "Cargo.toml");
  if (fs.existsSync(cargo)) crates.push(parseCargoToml(cargo));
}

if (crates.length === 0) {
  console.log("skip native architecture guard: no crate manifests found yet (Phase 0).");
  process.exit(0);
}

const byName = new Map(crates.map((c) => [c.name, c]));
const edgeViolations = [];
const unknownCrates = [];

for (const crate of crates) {
  if (!(crate.name in ALLOWED_EDGES)) {
    unknownCrates.push(`${crate.name}: new crate must be registered in ALLOWED_EDGES (docs section 5.7)`);
    continue;
  }
  const allowed = new Set(ALLOWED_EDGES[crate.name]);
  for (const dep of crate.deps) {
    if (!byName.has(dep)) continue;
    if (!allowed.has(dep)) {
      edgeViolations.push(`${crate.name} -> ${dep}: not allowed (allowed: ${[...allowed].join(", ") || "none"})`);
    }
  }
}

const graph = new Map();
for (const crate of crates) {
  graph.set(crate.name, crate.deps.filter((d) => byName.has(d)));
}
const cycles = [];
const state = new Map();
function visit(node, stack) {
  const s = state.get(node) || 0;
  if (s === 1) {
    const start = stack.indexOf(node);
    cycles.push([...stack.slice(start), node].join(" -> "));
    return;
  }
  if (s === 2) return;
  state.set(node, 1);
  stack.push(node);
  for (const next of graph.get(node) || []) visit(next, stack);
  stack.pop();
  state.set(node, 2);
}
for (const crate of crates) visit(crate.name, []);

const symbolViolations = [];
for (const crate of crates) {
  const patterns = FORBIDDEN_SYMBOLS[crate.name];
  if (!patterns) continue;
  const files = walk(crate.dir, (f) => f.endsWith(".rs"));
  for (const file of files) {
    const rel = path.relative(nativeRoot, file).split(path.sep).join("/");
    const src = fs.readFileSync(file, "utf8");
    for (const re of patterns) {
      const m = src.match(re);
      if (m) {
        symbolViolations.push(`${rel}: forbidden symbol "${m[0].trim()}" in ${crate.name}`);
        break;
      }
    }
  }
}

const sizeViolations = [];
for (const file of walk(nativeRoot, (f) => f.endsWith(".rs"))) {
  const rel = path.relative(nativeRoot, file).split(path.sep).join("/");
  const lines = fs.readFileSync(file, "utf8").split(/\r?\n/).length;
  if (lines > HARD_CAP) sizeViolations.push(`${rel}: ${lines} lines > hard cap ${HARD_CAP}`);
}

const sections = [
  ["unknown crates", unknownCrates],
  ["dependency edges", edgeViolations],
  ["dependency cycles", cycles],
  ["layer symbol leaks", symbolViolations],
  ["file-size debt", sizeViolations],
];

let failed = false;
for (const [name, list] of sections) {
  if (list.length) {
    failed = true;
    console.log(`\nX ${name} (${list.length}):`);
    for (const item of list) console.log("  - " + item);
  } else {
    console.log("ok " + name + ": ok");
  }
}

if (failed) {
  console.log("\nNative architecture guard FAILED. Fix the violations above; exceptions require an ADR.");
  process.exit(1);
}
console.log(`\nNative architecture guard passed (${crates.length} crates).`);
