// Native crate layering rules, as a pure function over a directory tree.
//
// Split out of check-native-architecture.mjs so the rules can be exercised
// against fixture workspaces (see ../check-native-architecture.selfcheck.mjs)
// instead of only against the one real tree in native/. The CLI keeps the
// console formatting and the exit code; this module owns only the judgement.
//
// Rules (ADR-0009, plan 2026-11-24-native-runtime-redesign section 5.7):
//   1) crate dependency edges must follow ALLOWED_EDGES (no cycles)
//   2) platform / UI / storage / network crates must not reach the wrong layer,
//      neither as a `crate::` path in source nor as a line in Cargo.toml
//   3) every .rs file stays <= HARD_CAP lines
//   4) a crate that is not registered here is itself a violation, so adding a
//      layer requires a deliberate edit to this file.

import fs from "node:fs";
import path from "node:path";

export const HARD_CAP = 800;

export const ALLOWED_EDGES = {
  app: ["hub-ui", "hub-editor", "hub-win", "hub-core", "hub-bridge"],
  "hub-ui": ["hub-editor", "hub-win", "hub-core"],
  "hub-editor": ["hub-win", "hub-core"],
  "hub-bridge": ["hub-core"],
  "hub-win": [],
  "hub-core": [],
};

// One list per crate, used twice: to build the source-scan patterns below and to
// reject the same crate declared in Cargo.toml. Two separate tables would let the
// manifest and the source side drift apart, and a banned crate that is declared
// but not yet called is still a banned crate -- it is one `use` away from being a
// leak, and the whole point of the gate is to fail before that `use` is written.
export const BANNED_DEPS = {
  app: ["tauri"],
  "hub-core": ["egui", "eframe", "windows", "windows-sys", "tauri", "reqwest"],
  "hub-ui": ["windows", "windows-sys", "rusqlite", "reqwest", "tauri"],
  "hub-editor": ["egui", "eframe", "rusqlite", "tauri"],
  "hub-win": ["egui", "eframe", "rusqlite", "tauri"],
  "hub-bridge": ["egui", "eframe", "windows", "windows-sys", "rusqlite", "tauri"],
};

// `windows-sys` is the raw FFI crate and `windows` the typed one; a layer that
// must not touch Win32 must not touch either. The source form of a crate name
// uses underscores, so both are matched from the manifest spelling.
const sourceForm = (name) => name.replace(/-/g, "_");
const reEscape = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

function familyPattern(names) {
  const alts = names.map(sourceForm).map(reEscape).sort((a, b) => b.length - a.length);
  return alts.join("|");
}

function symbolPatterns(names) {
  const family = familyPattern(names);
  return [
    new RegExp(`\\b(?:${family})::`),
    // A bare `use egui;` has no path to match, so it needs its own line rule.
    new RegExp(`^\\s*use\\s+(?:${family})\\b`, "m"),
  ];
}

// Patterns that predate BANNED_DEPS and are broader on purpose. `app` has always
// rejected any mention of tauri anywhere in its source, comments included;
// narrowing that to `tauri::` would quietly relax a rule CI has been enforcing.
const EXTRA_SYMBOL_PATTERNS = {
  app: [/tauri/],
};

export const FORBIDDEN_SYMBOLS = Object.fromEntries(
  Object.entries(BANNED_DEPS).map(([crate, names]) => [
    crate,
    [...symbolPatterns(names), ...(EXTRA_SYMBOL_PATTERNS[crate] || [])],
  ]),
);

export const normalize = (name) => name.replace(/_/g, "-");

export function walk(dir, predicate, out = []) {
  if (!fs.existsSync(dir)) return out;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (entry.name === "target" || entry.name === ".git") continue;
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, predicate, out);
    else if (predicate(full)) out.push(full);
  }
  return out;
}

export function parseCargoToml(file) {
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
      // `[workspace.dependencies]` lists shared versions for member crates; it is
      // not this crate's dependency list, so it must not be counted as one.
      if (/^workspace\b/.test(section)) {
        section = "";
        continue;
      }
      // `[dependencies]` / `[dev-dependencies]` / `[build-dependencies]`, anywhere
      // in the section path -- `[target.'cfg(windows)'.dependencies]` included,
      // otherwise a banned crate can hide behind a cfg.
      const tableDep = section.match(/(?:^|\.)(?:dev-|build-)?dependencies\.(.+)$/);
      if (tableDep) deps.add(normalize(tableDep[1]));
      continue;
    }

    if (/(?:^|\.)(?:dev-|build-)?dependencies$/.test(section)) {
      const kv = line.match(/^([A-Za-z0-9_-]+)\s*=/);
      if (kv) deps.add(normalize(kv[1]));
    }
  }

  return {
    name: nameMatch ? normalize(nameMatch[1]) : normalize(path.basename(dir)),
    dir,
    manifest: file,
    deps: [...deps],
  };
}

export function collectCrates(nativeRoot) {
  const crateDirs = [];
  const cratesRoot = path.join(nativeRoot, "crates");
  if (fs.existsSync(cratesRoot)) {
    for (const entry of fs.readdirSync(cratesRoot, { withFileTypes: true })) {
      if (entry.isDirectory()) crateDirs.push(path.join(cratesRoot, entry.name));
    }
  }
  const appDir = path.join(nativeRoot, "app");
  if (fs.existsSync(appDir)) crateDirs.push(appDir);

  const crates = [];
  for (const dir of crateDirs) {
    const cargo = path.join(dir, "Cargo.toml");
    if (fs.existsSync(cargo)) crates.push(parseCargoToml(cargo));
  }
  return crates;
}

/**
 * @param {string} nativeRoot absolute path to native/
 * @returns {{
 *   skipped: boolean, reason: string|null, crates: string[],
 *   unknownCrates: string[], edgeViolations: string[], cycles: string[],
 *   symbolViolations: string[], dependencyViolations: string[],
 *   sizeViolations: string[],
 *   sections: Array<{name: string, items: string[]}>, failed: boolean
 * }}
 */
export function analyzeNativeArchitecture(nativeRoot) {
  const empty = {
    skipped: true,
    reason: null,
    crates: [],
    unknownCrates: [],
    edgeViolations: [],
    cycles: [],
    symbolViolations: [],
    dependencyViolations: [],
    sizeViolations: [],
    sections: [],
    failed: false,
  };

  if (!fs.existsSync(nativeRoot)) {
    return { ...empty, reason: "native/ not created yet (Phase 0)." };
  }

  const crates = collectCrates(nativeRoot);
  if (crates.length === 0) {
    return { ...empty, reason: "no crate manifests found yet (Phase 0)." };
  }

  const byName = new Map(crates.map((c) => [c.name, c]));
  const unknownCrates = [];
  const edgeViolations = [];

  for (const crate of crates) {
    if (!(crate.name in ALLOWED_EDGES)) {
      unknownCrates.push(
        `${crate.name}: new crate must be registered in ALLOWED_EDGES (docs section 5.7)`,
      );
      continue;
    }
    const allowed = new Set(ALLOWED_EDGES[crate.name]);
    for (const dep of crate.deps) {
      if (!byName.has(dep)) continue;
      if (!allowed.has(dep)) {
        edgeViolations.push(
          `${crate.name} -> ${dep}: not allowed (allowed: ${[...allowed].join(", ") || "none"})`,
        );
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

  // Declared-but-unused is still a leak in waiting: the next `use` is the only
  // remaining step, and it may well be written by someone who never reads this
  // file. Rejecting the declaration keeps the layer boundary in one place.
  const dependencyViolations = [];
  for (const crate of crates) {
    const banned = BANNED_DEPS[crate.name];
    if (!banned) continue;
    const set = new Set(banned);
    const rel = path.relative(nativeRoot, crate.manifest).split(path.sep).join("/");
    for (const dep of crate.deps) {
      if (set.has(dep)) {
        dependencyViolations.push(`${rel}: forbidden dependency "${dep}" in ${crate.name}`);
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
    { name: "unknown crates", items: unknownCrates },
    { name: "dependency edges", items: edgeViolations },
    { name: "dependency cycles", items: cycles },
    { name: "layer symbol leaks", items: symbolViolations },
    { name: "forbidden dependencies", items: dependencyViolations },
    { name: "file-size debt", items: sizeViolations },
  ];

  return {
    skipped: false,
    reason: null,
    crates: crates.map((c) => c.name),
    unknownCrates,
    edgeViolations,
    cycles,
    symbolViolations,
    dependencyViolations,
    sizeViolations,
    sections,
    failed: sections.some((s) => s.items.length > 0),
  };
}
