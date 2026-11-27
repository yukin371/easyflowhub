#!/usr/bin/env node
// Documentation budget guard.
//
// Why this exists: `docs/roadmap.md` reached 90 KB because every session appended a
// dated entry to it, which is exactly what ADR-0001 forbids ("roadmap 只维护当前版本
// 目标、活跃轨道、验证债和下一队列，不保留长历史"). A size number alone would only
// catch the symptom after the damage, so the rules attack the shape as well:
//
//   1. single-file budget -- a hard cap fails outright; files between the soft cap and
//      the hard cap are pinned in `scripts/doc-budget.json` and may only shrink. Growth
//      of a pinned file fails, which is the ratchet: you cannot keep appending.
//   2. relative links -- moving a section into its own file is how links rot. Every
//      `](./x)` must resolve, so a split is verified by the guard instead of by eye.
//   3. roadmap shape -- dated progress entries belong in
//      `docs/roadmap/progress-YYYY-MM.md`; the route document keeps the index and must
//      link every month file that exists.
//
// Usage:
//   node scripts/check-doc-budget.mjs [--root <dir>] [--pin]
//
// `--pin` rewrites the pinned sizes from the current files. It refuses to raise a pin,
// so `--pin` tightens the ratchet and can never be used to launder growth.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const argv = process.argv.slice(2);
const rootFlag = argv.indexOf('--root');
const ROOT = rootFlag >= 0 ? path.resolve(argv[rootFlag + 1]) : path.resolve(HERE, '..');
const PIN = argv.includes('--pin');

const BUDGET_PATH = path.join(ROOT, 'scripts', 'doc-budget.json');

function walkMarkdown(dir, out = []) {
  if (!fs.existsSync(dir)) return out;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walkMarkdown(full, out);
    else if (entry.name.endsWith('.md')) out.push(full);
  }
  return out;
}

function rel(file) {
  return path.relative(ROOT, file).split(path.sep).join('/');
}

function kb(bytes) {
  return `${(bytes / 1024).toFixed(1)} KB`;
}

function loadBudget() {
  if (!fs.existsSync(BUDGET_PATH)) return null;
  const raw = JSON.parse(fs.readFileSync(BUDGET_PATH, 'utf8'));
  for (const key of ['softBytes', 'hardBytes']) {
    if (typeof raw[key] !== 'number' || raw[key] <= 0) {
      throw new Error(`${rel(BUDGET_PATH)}: \`${key}\` must be a positive number`);
    }
  }
  raw.pinnedBytes ??= {};
  return raw;
}

function writeBudget(budget) {
  const sorted = {};
  for (const key of Object.keys(budget.pinnedBytes).sort()) {
    sorted[key] = budget.pinnedBytes[key];
  }
  const text = `${JSON.stringify({ ...budget, pinnedBytes: sorted }, null, 2)}\n`;
  fs.writeFileSync(BUDGET_PATH, text, 'utf8');
}

const files = [
  ...walkMarkdown(path.join(ROOT, 'docs')),
  ...['README.md', 'AGENTS.md', path.join('native', 'README.md')]
    .map((name) => path.join(ROOT, name))
    .filter((file) => fs.existsSync(file)),
];
files.sort();

const budget = loadBudget();
if (!budget) {
  console.error(`FAIL doc budget: ${rel(BUDGET_PATH)} is missing; create it with --pin`);
  process.exit(1);
}

const failures = [];

// ---------------------------------------------------------------- 1. size budget
const sizes = new Map();
for (const file of files) {
  sizes.set(rel(file), fs.statSync(file).size);
}

if (PIN) {
  const pinned = {};
  const raised = [];
  for (const [name, bytes] of sizes) {
    if (bytes <= budget.softBytes) continue;
    const previous = budget.pinnedBytes[name];
    if (typeof previous === 'number' && bytes > previous) raised.push(`${name} ${kb(previous)} -> ${kb(bytes)}`);
    pinned[name] = bytes;
  }
  if (raised.length > 0) {
    console.error('FAIL doc budget: --pin cannot raise a pinned size; split the file instead:');
    for (const line of raised) console.error(`  ${line}`);
    process.exit(1);
  }
  writeBudget({ ...budget, pinnedBytes: pinned });
  console.log(`doc budget: pinned ${Object.keys(pinned).length} file(s) over the soft cap`);
  for (const [name, bytes] of Object.entries(pinned).sort()) {
    console.log(`  ${kb(bytes)}  ${name}`);
  }
  process.exit(0);
}

for (const [name, bytes] of sizes) {
  if (bytes > budget.hardBytes) {
    failures.push(`${name}: ${kb(bytes)} is over the hard cap (${kb(budget.hardBytes)}) -- split it into layers`);
    continue;
  }
  if (bytes <= budget.softBytes) continue;
  const pinned = budget.pinnedBytes[name];
  if (typeof pinned !== 'number') {
    failures.push(
      `${name}: ${kb(bytes)} is over the soft cap (${kb(budget.softBytes)}) and not pinned -- split it, or pin it deliberately with --pin`,
    );
  } else if (bytes > pinned) {
    failures.push(`${name}: grew ${kb(pinned)} -> ${kb(bytes)} -- this file is closed for growth; split it`);
  }
}
const unpinned = Object.keys(budget.pinnedBytes).filter((name) => !sizes.has(name));
for (const name of unpinned) {
  failures.push(`${name}: pinned but no longer exists -- drop it with --pin`);
}
for (const [name, bytes] of sizes) {
  const pinned = budget.pinnedBytes[name];
  if (typeof pinned === 'number' && bytes <= budget.softBytes) {
    failures.push(`${name}: ${kb(bytes)} is back under the soft cap -- tighten the ratchet with --pin`);
  } else if (typeof pinned === 'number' && bytes < pinned) {
    // Shrinking is good; it only needs a pin refresh so the ratchet stays tight.
    console.log(`note ${name}: ${kb(pinned)} -> ${kb(bytes)}; run --pin to tighten`);
  }
}

// ---------------------------------------------------------------- 2. relative links
const SCHEME = /^[a-z][a-z0-9+.-]*:/i;

// Links are judged in prose only: a file name inside backticks is documentation about
// a link, not a link. (This bit is load-bearing: the roadmap's own entry explaining
// the `](./x)` -> `](../x)` rewrite must not be read as two broken links.)
function prose(text) {
  return text.replace(/```[\s\S]*?```/g, '').replace(/`[^`\n]*`/g, '');
}

let linkCount = 0;
for (const file of files) {
  const text = prose(fs.readFileSync(file, 'utf8'));
  const linkPattern = /\]\(([^)\s]+)\)/g;
  for (const match of text.matchAll(linkPattern)) {
    const raw = match[1];
    if (raw.startsWith('#') || SCHEME.test(raw)) continue;
    const target = raw.split('#')[0];
    if (!target) continue;
    linkCount += 1;
    const resolved = path.resolve(path.dirname(file), decodeURIComponent(target));
    if (!fs.existsSync(resolved)) {
      failures.push(`${rel(file)}: link does not resolve -> ${raw}`);
    }
  }
}

// ---------------------------------------------------------------- 3. roadmap shape
const ROADMAP = path.join(ROOT, 'docs', 'roadmap.md');
const PROGRESS_DIR = path.join(ROOT, 'docs', 'roadmap');
if (fs.existsSync(ROADMAP)) {
  const text = fs.readFileSync(ROADMAP, 'utf8');
  const dated = text.split('\n').filter((line) => /^- `\d{4}-\d{2}-\d{2}`/.test(line));
  if (dated.length > 0) {
    failures.push(
      `${rel(ROADMAP)}: ${dated.length} dated progress entries -- move them to docs/roadmap/progress-YYYY-MM.md and leave a link (ADR-0001)`,
    );
  }
  if (fs.existsSync(PROGRESS_DIR)) {
    for (const entry of fs.readdirSync(PROGRESS_DIR, { withFileTypes: true })) {
      // Two kinds of layer live here: monthly progress logs and frozen released
      // versions. Anything else is a file that escaped the convention.
      if (!entry.isDirectory() && !/^(progress-\d{4}-\d{2}|v\d+\.\d+\.\d+)\.md$/.test(entry.name)) {
        failures.push(
          `${rel(path.join(PROGRESS_DIR, entry.name))}: route layers must be progress-YYYY-MM.md or vX.Y.Z.md`,
        );
        continue;
      }
      const linked = text.includes(`roadmap/${entry.name}`) || text.includes(`./roadmap/${entry.name}`);
      if (!linked) failures.push(`${rel(path.join(PROGRESS_DIR, entry.name))}: not linked from docs/roadmap.md`);
    }
  }
}

// ---------------------------------------------------------------- report
if (failures.length > 0) {
  for (const line of failures) console.error(`FAIL ${line}`);
  console.error(`Doc budget guard failed (${failures.length} problem(s), ${files.length} files)`);
  process.exit(1);
}

const largest = [...sizes.entries()].sort((a, b) => b[1] - a[1])[0];
console.log(`ok size budget: ok (soft ${kb(budget.softBytes)}, hard ${kb(budget.hardBytes)}, ${Object.keys(budget.pinnedBytes).length} pinned)`);
console.log(`ok relative links: ok (${linkCount} checked)`);
console.log('ok roadmap shape: ok (no dated entries, every log linked)');
console.log(
  `Doc budget guard passed (${files.length} files; largest ${largest[0]} at ${kb(largest[1])}).`,
);
