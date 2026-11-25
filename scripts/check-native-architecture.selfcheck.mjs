// Self-check for the native architecture guard.
//
// A gate that has never been seen to fail is not evidence of anything: the real
// tree is correct, so every rule could be broken and the guard would still print
// PASS. Each test below builds a throwaway workspace in the OS temp directory,
// breaks exactly one rule, and asserts the specific violation comes back -- plus
// a set of "must not fire" cases, because a guard that also rejects legal code
// gets exemptions bolted on until it means nothing.
//
// Run: node --test scripts/check-native-architecture.selfcheck.mjs

import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { analyzeNativeArchitecture, HARD_CAP } from "./lib/native-arch-guard.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(here, "..");

// ---------------------------------------------------------------- fixtures ---

// A workspace that satisfies every rule. Each violation case clones this and
// changes one thing, so a failure can only come from that change.
const BASELINE = [
  {
    name: "app",
    deps: ["hub-ui", "hub-editor", "hub-win", "hub-core", "egui", "eframe"],
    files: { "main.rs": "fn main() {}\n" },
  },
  {
    name: "hub-ui",
    deps: ["egui", "hub-core"],
    files: { "lib.rs": "use egui::Context;\n\npub fn noop(_: &Context) {}\n" },
  },
  {
    name: "hub-editor",
    deps: ["hub-core", "windows-sys"],
    files: { "lib.rs": "use windows_sys::Win32::Foundation::HWND;\n" },
  },
  { name: "hub-win", deps: ["windows-sys"], files: { "lib.rs": "pub fn noop() {}\n" } },
  { name: "hub-core", deps: ["rusqlite"], files: { "lib.rs": "pub fn noop() {}\n" } },
];

const clone = () => JSON.parse(JSON.stringify(BASELINE));

function crateOf(spec, name) {
  const found = spec.find((c) => c.name === name);
  assert.ok(found, `fixture is missing crate ${name}`);
  return found;
}

// `split(/\r?\n/)` is what the guard counts, so build the file to that measure:
// N joined lines with no trailing newline splits into exactly N entries.
function fileWithLineCount(n) {
  return Array.from({ length: n }, (_, i) => `// line ${i + 1}`).join("\n");
}

function renderManifest(crate) {
  const lines = ["[package]", `name = "${crate.name}"`, 'version = "0.1.0"', ""];
  if (crate.manifestExtra) lines.push(crate.manifestExtra.trimEnd(), "");
  if (crate.deps.length) {
    lines.push("[dependencies]");
    for (const dep of crate.deps) lines.push(`${dep} = "*"`);
    lines.push("");
  }
  return lines.join("\n");
}

function materialize(spec) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "hub-arch-"));
  for (const crate of spec) {
    const dir =
      crate.name === "app" ? path.join(root, "app") : path.join(root, "crates", crate.name);
    const src = path.join(dir, "src");
    fs.mkdirSync(src, { recursive: true });
    fs.writeFileSync(path.join(dir, "Cargo.toml"), renderManifest(crate));
    for (const [rel, body] of Object.entries(crate.files || { "lib.rs": "" })) {
      const file = path.join(src, rel);
      fs.mkdirSync(path.dirname(file), { recursive: true });
      fs.writeFileSync(file, body);
    }
  }
  return root;
}

// Materialize, analyze, always clean up (Windows keeps handles longer, so retry).
function check(t, spec) {
  const root = materialize(spec);
  t.after(() => {
    try {
      fs.rmSync(root, { recursive: true, force: true, maxRetries: 5, retryDelay: 50 });
    } catch {
      /* a leftover temp dir must not fail the check */
    }
  });
  return analyzeNativeArchitecture(root);
}

// ------------------------------------------------------------- the baseline ---

test("a workspace that follows the matrix produces no violations", (t) => {
  const r = check(t, clone());
  assert.equal(r.skipped, false);
  assert.deepEqual(r.crates.sort(), ["app", "hub-core", "hub-editor", "hub-ui", "hub-win"]);
  assert.deepEqual(r.unknownCrates, []);
  assert.deepEqual(r.edgeViolations, []);
  assert.deepEqual(r.cycles, []);
  assert.deepEqual(r.symbolViolations, []);
  assert.deepEqual(r.dependencyViolations, []);
  assert.deepEqual(r.sizeViolations, []);
  assert.equal(r.failed, false);
  assert.equal(r.sections.length, 6);
});

test("the real native/ tree passes, so this self-check is not testing a fiction", () => {
  const r = analyzeNativeArchitecture(path.join(repoRoot, "native"));
  assert.equal(r.skipped, false);
  assert.equal(r.failed, false, JSON.stringify(r.sections, null, 2));
});

// ------------------------------------------------------- class 1: the edges ---

test("class 1: an edge outside the matrix is reported", (t) => {
  const spec = clone();
  crateOf(spec, "hub-win").deps.push("hub-core");
  const r = check(t, spec);
  assert.equal(r.edgeViolations.length, 1);
  assert.match(r.edgeViolations[0], /^hub-win -> hub-core: not allowed \(allowed: none\)$/);
  assert.equal(r.failed, true);
});

test("class 1: an edge declared in [dependencies.foo] table form is also caught", (t) => {
  const spec = clone();
  const win = crateOf(spec, "hub-win");
  win.deps = ["windows-sys"];
  win.manifestExtra = "[dependencies.hub-core]\npath = \"../hub-core\"";
  const r = check(t, spec);
  assert.equal(r.edgeViolations.length, 1);
  assert.match(r.edgeViolations[0], /^hub-win -> hub-core: not allowed/);
});

test("class 1: a two-node cycle is reported once, independent of the edge report", (t) => {
  const spec = clone();
  crateOf(spec, "hub-core").deps.push("hub-ui");
  const r = check(t, spec);
  // The backwards edge is itself illegal, so both signals fire -- but the cycle
  // list must name the loop, not just repeat the edge violation.
  assert.equal(r.cycles.length, 1);
  const parts = r.cycles[0].split(" -> ");
  assert.equal(parts[0], parts.at(-1), `cycle must close: ${r.cycles[0]}`);
  // The loop is hub-core <-> hub-ui. Which node the DFS starts from depends on
  // directory order, so assert the members rather than a fixed rotation.
  assert.deepEqual([...new Set(parts)].sort(), ["hub-core", "hub-ui"]);
});

test("a crate that is not registered in ALLOWED_EDGES is rejected outright", (t) => {
  const spec = clone();
  spec.push({ name: "hub-extra", deps: ["hub-core"], files: { "lib.rs": "" } });
  const r = check(t, spec);
  assert.equal(r.unknownCrates.length, 1);
  assert.match(r.unknownCrates[0], /^hub-extra: new crate must be registered in ALLOWED_EDGES/);
});

// ------------------------------------------------ class 2: symbol leakage ----

test("class 2: egui in hub-core is reported", (t) => {
  const spec = clone();
  crateOf(spec, "hub-core").files["lib.rs"] = "use egui::Context;\n";
  const r = check(t, spec);
  assert.equal(r.symbolViolations.length, 1);
  assert.match(r.symbolViolations[0], /^crates\/hub-core\/src\/lib\.rs: forbidden symbol ".*egui.*" in hub-core$/);
});

test("class 2: windows in hub-ui is reported", (t) => {
  const spec = clone();
  crateOf(spec, "hub-ui").files["lib.rs"] = "use windows::Win32::Foundation::HWND;\n";
  const r = check(t, spec);
  assert.equal(r.symbolViolations.length, 1);
  assert.match(r.symbolViolations[0], /hub-ui$/);
});

test("class 2 must NOT fire: egui in hub-ui is where it belongs", (t) => {
  const r = check(t, clone());
  assert.deepEqual(r.symbolViolations, []);
});

test("class 2 must NOT fire: windows_sys in hub-editor and hub-win is their job", (t) => {
  const spec = clone();
  crateOf(spec, "hub-win").files["lib.rs"] =
    "use windows_sys::Win32::UI::WindowsAndMessaging::HWND;\n";
  const r = check(t, spec);
  assert.deepEqual(r.symbolViolations, []);
});

// Known limitation, pinned as an executable fact rather than left to memory:
// the scan is textual, so a `crate::` path inside a comment counts. Removing
// comments correctly (raw strings, char literals, nested blocks) is its own
// source of misses, and a missed leak is worse than a noisy one -- so the rule
// stays conservative and the message names the file so the fix is obvious.
test("known limitation: a crate:: path inside a doc comment is reported", (t) => {
  const spec = clone();
  crateOf(spec, "hub-core").files["lib.rs"] = "//! never reach for egui::Context here\npub fn noop() {}\n";
  const r = check(t, spec);
  assert.equal(r.symbolViolations.length, 1);
});

// The three cases below were found by writing the tests above, not by reading
// the rule: each one is a way to get a banned crate into a layer without ever
// writing `banned::` in source, which is all the original scan looked at.
test("class 2: hub-core importing windows_sys is reported (it is the same platform family)", (t) => {
  const spec = clone();
  crateOf(spec, "hub-core").deps.push("windows-sys");
  crateOf(spec, "hub-core").files["lib.rs"] = "use windows_sys::Win32::Foundation::HWND;\n";
  const r = check(t, spec);
  assert.equal(r.symbolViolations.length, 1);
  assert.match(r.symbolViolations[0], /windows_sys.*in hub-core$/);
});

test("class 2: a banned crate declared in Cargo.toml is reported even when unused", (t) => {
  const spec = clone();
  crateOf(spec, "hub-core").deps.push("egui");
  const r = check(t, spec);
  assert.equal(r.dependencyViolations.length, 1);
  assert.match(r.dependencyViolations[0], /hub-core.*forbidden dependency "egui"/);
});

test("class 2: a banned crate under a target-specific table is still found", (t) => {
  const spec = clone();
  const core = crateOf(spec, "hub-core");
  core.manifestExtra = "[target.'cfg(windows)'.dependencies]\nwindows-sys = \"*\"";
  const r = check(t, spec);
  assert.equal(r.dependencyViolations.length, 1);
  assert.match(r.dependencyViolations[0], /hub-core.*forbidden dependency "windows-sys"/);
});

test("class 2: a banned crate in [target...dependencies.foo] table form is still found", (t) => {
  const spec = clone();
  const core = crateOf(spec, "hub-core");
  core.manifestExtra = "[target.'cfg(windows)'.dependencies.windows-sys]\nversion = \"*\"";
  const r = check(t, spec);
  assert.equal(r.dependencyViolations.length, 1);
  assert.match(r.dependencyViolations[0], /hub-core.*forbidden dependency "windows-sys"/);
});

test("class 2 must NOT fire: rusqlite in hub-core and workspace-level tables", (t) => {
  const spec = clone();
  // A workspace-level dependency table is not this crate's dependency list.
  crateOf(spec, "hub-core").manifestExtra = "[workspace.dependencies]\negui = \"*\"";
  const r = check(t, spec);
  assert.deepEqual(r.dependencyViolations, []);
});

// ------------------------------------------------------- class 3: the size ---

// The cap is pinned as a literal, not read from the module: an earlier version
// of this file derived the fixture size from the exported HARD_CAP, so raising
// the cap moved the fixture with it and the mutation survived.
test("the hard cap is the number the design documents (800)", () => {
  assert.equal(HARD_CAP, 800);
});

test("class 3: a file over the hard cap is reported", (t) => {
  const spec = clone();
  crateOf(spec, "hub-win").files["big.rs"] = fileWithLineCount(801);
  const r = check(t, spec);
  assert.equal(r.sizeViolations.length, 1);
  assert.match(r.sizeViolations[0], /^crates\/hub-win\/src\/big\.rs: 801 lines > hard cap 800$/);
});

test("class 3 boundary: exactly the hard cap is allowed", (t) => {
  const spec = clone();
  crateOf(spec, "hub-win").files["big.rs"] = fileWithLineCount(800);
  const r = check(t, spec);
  assert.deepEqual(r.sizeViolations, []);
});

test("class 3: target/ is not scanned, so build output cannot fail the gate", (t) => {
  const spec = clone();
  crateOf(spec, "hub-win").files["../../target/generated.rs"] = fileWithLineCount(830);
  const r = check(t, spec);
  assert.deepEqual(r.sizeViolations, []);
});

// ------------------------------------------------------------- skip branch ---

test("skip: no native/ directory", (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "hub-arch-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const r = analyzeNativeArchitecture(path.join(root, "native"));
  assert.equal(r.skipped, true);
  assert.match(r.reason, /native\/ not created yet/);
  assert.equal(r.failed, false);
});

test("skip: native/ exists but holds no manifest yet", (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "hub-arch-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  fs.mkdirSync(path.join(root, "native", "crates"), { recursive: true });
  const r = analyzeNativeArchitecture(path.join(root, "native"));
  assert.equal(r.skipped, true);
  assert.match(r.reason, /no crate manifests found/);
});

// External crates that merely appear in the manifest are not edges between our
// crates, so they must not be reported as "unknown crate" or as edge violations.
test("external dependencies are not mistaken for internal edges", (t) => {
  const r = check(t, clone());
  assert.ok(r.crates.includes("hub-core"));
  assert.ok(!r.crates.includes("rusqlite"));
  assert.deepEqual(r.unknownCrates, []);
});
