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
// The judgement lives in lib/native-arch-guard.mjs so it can be tested against
// fixture trees; this file only formats the result and owns the exit code.
//
// Usage: node scripts/check-native-architecture.mjs

import path from "node:path";
import { fileURLToPath } from "node:url";

import { analyzeNativeArchitecture } from "./lib/native-arch-guard.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(here, "..");
const nativeRoot = path.join(repoRoot, "native");

const result = analyzeNativeArchitecture(nativeRoot);

if (result.skipped) {
  console.log(`skip native architecture guard: ${result.reason}`);
  process.exit(0);
}

for (const { name, items } of result.sections) {
  if (items.length) {
    console.log(`\nX ${name} (${items.length}):`);
    for (const item of items) console.log("  - " + item);
  } else {
    console.log("ok " + name + ": ok");
  }
}

if (result.failed) {
  console.log("\nNative architecture guard FAILED. Fix the violations above; exceptions require an ADR.");
  process.exit(1);
}
console.log(`\nNative architecture guard passed (${result.crates.length} crates).`);
