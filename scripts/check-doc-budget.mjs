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
//   3. journal shape -- dated entries live under `docs/roadmap/journal/`, three layers
//      deep (month -> week -> log). The key is time, not bytes: every log must be linked
//      from its week, every week from its month, every month from the route document.
//      A file named W48 may hold no date outside that week. Early months that predate the
//      split are single-layer and stay legal -- the rule is "if it exists it is linked",
//      not "every month must have children". This judges index integrity, which is the
//      only thing that makes a layered log searchable.
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
  for (const layer of raw.layers ?? []) {
    if (typeof layer.match !== 'string' || layer.match.length === 0) {
      throw new Error(`${rel(BUDGET_PATH)}: every layer needs a \`match\` path prefix`);
    }
    // `softBytes: null` marks an append-only layer: its size is set by how much happened,
    // so a soft cap there would only reward deleting the entries it exists to keep.
    if (layer.softBytes !== null && (typeof layer.softBytes !== 'number' || layer.softBytes <= 0)) {
      throw new Error(`${rel(BUDGET_PATH)}: layer ${layer.match} needs \`softBytes\` as a positive number or null`);
    }
    if (typeof layer.hardBytes !== 'number' || layer.hardBytes <= 0) {
      throw new Error(`${rel(BUDGET_PATH)}: layer ${layer.match} needs \`hardBytes\` as a positive number`);
    }
  }
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

// A layer inherits the default budget unless a `layers` entry claims it by path prefix.
// `soft: null` means "no ratchet here": the append-only log layer.
function limitsFor(name) {
  for (const layer of budget.layers ?? []) {
    if (name.startsWith(layer.match)) {
      return { soft: layer.softBytes, hard: layer.hardBytes };
    }
  }
  return { soft: budget.softBytes, hard: budget.hardBytes };
}

if (PIN) {
  const pinned = {};
  const raised = [];
  for (const [name, bytes] of sizes) {
    const { soft } = limitsFor(name);
    if (soft === null || bytes <= soft) continue;
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
  const { soft, hard } = limitsFor(name);
  if (bytes > hard) {
    failures.push(`${name}: ${kb(bytes)} is over the hard cap (${kb(hard)}) -- split it into layers`);
    continue;
  }
  if (soft === null) continue; // append-only layer: growth is the point, not the problem
  if (bytes <= soft) continue;
  const pinned = budget.pinnedBytes[name];
  if (typeof pinned !== 'number') {
    failures.push(
      `${name}: ${kb(bytes)} is over the soft cap (${kb(soft)}) and not pinned -- split it, or pin it deliberately with --pin`,
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
  const { soft } = limitsFor(name);
  if (soft === null) continue;
  const pinned = budget.pinnedBytes[name];
  if (typeof pinned === 'number' && bytes <= soft) {
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

// ---------------------------------------------------------------- 3. journal shape
const ROADMAP = path.join(ROOT, 'docs', 'roadmap.md');
const ROADMAP_DIR = path.join(ROOT, 'docs', 'roadmap');
const JOURNAL = path.join(ROADMAP_DIR, 'journal');

// The Thursday decides both the ISO week-year and the month a week belongs to. Using
// the Monday instead would push the last week of a month back into the previous one,
// and then a week file would name a month that contains none of its days.
function isoWeekOf(iso) {
  const [y, m, d] = iso.split('-').map(Number);
  const thursday = new Date(Date.UTC(y, m - 1, d));
  thursday.setUTCDate(thursday.getUTCDate() - ((thursday.getUTCDay() + 6) % 7) + 3);
  const firstThursday = new Date(Date.UTC(thursday.getUTCFullYear(), 0, 4));
  firstThursday.setUTCDate(firstThursday.getUTCDate() - ((firstThursday.getUTCDay() + 6) % 7) + 3);
  return {
    year: thursday.getUTCFullYear(),
    week: 1 + Math.round((thursday - firstThursday) / 604800000),
  };
}

function thursdayOfWeek(year, week) {
  const jan4 = new Date(Date.UTC(year, 0, 4));
  const monday = new Date(jan4);
  monday.setUTCDate(jan4.getUTCDate() - ((jan4.getUTCDay() + 6) % 7) + (week - 1) * 7);
  const thursday = new Date(monday);
  thursday.setUTCDate(monday.getUTCDate() + 3);
  return thursday;
}

function entriesOf(dir) {
  return fs.existsSync(dir) ? fs.readdirSync(dir, { withFileTypes: true }) : [];
}

function readText(file) {
  return fs.existsSync(file) ? fs.readFileSync(file, 'utf8') : '';
}

function monthNameOf(thursday) {
  return `${thursday.getUTCFullYear()}-${String(thursday.getUTCMonth() + 1).padStart(2, '0')}.md`;
}

if (fs.existsSync(ROADMAP)) {
  const route = readText(ROADMAP);
  const dated = route.split('\n').filter((line) => /^- `\d{4}-\d{2}-\d{2}`/.test(line));
  if (dated.length > 0) {
    failures.push(
      `${rel(ROADMAP)}: ${dated.length} dated progress entries -- move them to docs/roadmap/journal/log/ and leave a link (ADR-0001)`,
    );
  }
  // Two things may sit beside the journal: the journal itself and frozen releases.
  for (const entry of entriesOf(ROADMAP_DIR)) {
    if (entry.isDirectory() && entry.name === 'journal') continue;
    if (!entry.isDirectory() && /^v\d+\.\d+\.\d+\.md$/.test(entry.name)) continue;
    failures.push(`${rel(path.join(ROADMAP_DIR, entry.name))}: the route layer holds only journal/ and vX.Y.Z.md`);
  }
}

if (fs.existsSync(JOURNAL)) {
  for (const entry of entriesOf(JOURNAL)) {
    if (entry.isFile() && entry.name === 'README.md') continue;
    if (entry.isDirectory() && ['month', 'week', 'log'].includes(entry.name)) continue;
    failures.push(`${rel(path.join(JOURNAL, entry.name))}: journal holds only README.md and month/ week/ log/`);
  }

  const route = readText(ROADMAP);
  const monthDir = path.join(JOURNAL, 'month');
  const weekDir = path.join(JOURNAL, 'week');
  const logDir = path.join(JOURNAL, 'log');

  for (const entry of entriesOf(monthDir)) {
    if (!entry.isFile()) continue;
    const file = path.join(monthDir, entry.name);
    if (!/^\d{4}-\d{2}\.md$/.test(entry.name)) {
      failures.push(`${rel(file)}: month files are YYYY-MM.md`);
      continue;
    }
    if (!route.includes(`journal/month/${entry.name}`)) {
      failures.push(`${rel(file)}: not linked from docs/roadmap.md`);
    }
  }

  for (const entry of entriesOf(weekDir)) {
    if (!entry.isFile()) continue;
    const file = path.join(weekDir, entry.name);
    const parsed = /^(\d{4})-W(\d{2})\.md$/.exec(entry.name);
    if (!parsed) {
      failures.push(`${rel(file)}: week files are YYYY-Www.md`);
      continue;
    }
    const monthFile = path.join(monthDir, monthNameOf(thursdayOfWeek(Number(parsed[1]), Number(parsed[2]))));
    if (!fs.existsSync(monthFile)) {
      failures.push(`${rel(file)}: its Thursday falls in ${path.basename(monthFile)}, which does not exist`);
    } else if (!readText(monthFile).includes(entry.name)) {
      failures.push(`${rel(file)}: not linked from ${rel(monthFile)}`);
    }
  }

  for (const entry of entriesOf(logDir)) {
    if (!entry.isFile()) continue;
    const file = path.join(logDir, entry.name);
    const parsed = /^(\d{4})-(\d{2})-(\d{2})\.md$/.exec(entry.name);
    if (!parsed) {
      failures.push(`${rel(file)}: log files are YYYY-MM-DD.md`);
      continue;
    }
    const { year, week } = isoWeekOf(`${parsed[1]}-${parsed[2]}-${parsed[3]}`);
    const weekFile = path.join(weekDir, `${year}-W${String(week).padStart(2, '0')}.md`);
    if (!fs.existsSync(weekFile)) {
      failures.push(
        `${rel(file)}: ${parsed[1]}-${parsed[2]}-${parsed[3]} is in ${path.basename(weekFile)}, which does not exist`,
      );
    } else if (!readText(weekFile).includes(entry.name)) {
      failures.push(`${rel(file)}: not linked from ${rel(weekFile)}`);
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
console.log(
  `ok size budget: ok (default soft ${kb(budget.softBytes)}, hard ${kb(budget.hardBytes)}, ${(budget.layers ?? []).length} layer rule(s), ${Object.keys(budget.pinnedBytes).length} pinned)`,
);
console.log(`ok relative links: ok (${linkCount} checked)`);
console.log('ok journal shape: ok (month -> week -> log, every node linked, no dated route entries)');
console.log(
  `Doc budget guard passed (${files.length} files; largest ${largest[0]} at ${kb(largest[1])}).`,
);
