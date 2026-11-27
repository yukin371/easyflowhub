#!/usr/bin/env node
// ADR lifecycle guard.
//
// Why this exists: `docs/plans` got a lifecycle because "a delivered plan and an
// abandoned one look identical" was a real problem; ADRs had the same problem and no
// gate at all. The 2026-11-27 sweep found the shape of that rot: one ADR amended by
// another with only a one-way pointer, two ADRs missing from the index, three
// references to paths or symbol names that had moved. None of it was a wrong decision
// -- it was bookkeeping nobody was obliged to do.
//
// The rules, each failing independently:
//
//   1. files & naming     -- `ADR-NNNN-<slug>.md` only; no loose files in the directory.
//   2. header fields      -- `status` (fixed vocabulary), `date`, `scope` are required.
//                            A free-text status is the same as no status.
//   3. numbering          -- unique, contiguous; a number is never recycled.
//   4. relations          -- `supersedes`/`superseded-by` and `amends`/`amended-by`
//                            must point at each other; `amends` must name a section,
//                            otherwise "partly replaced" and "wholly replaced" are
//                            indistinguishable to every future reader.
//   5. state coherence    -- `superseded` iff `superseded-by`, `amended` iff
//                            `amended-by`, `accepted` carries neither.
//   6. index              -- `README.md` lists every ADR with the same token and date.
//   7. path resolution    -- a backticked path in the body must still exist; a moved
//                            directory is exactly how an ADR starts lying.
//   8. review ledger      -- quarterly review, recorded with date + next date; overdue
//                            fails, and a review that starts more than
//                            `deepAfterOverdueDays` late must be a deep one.
//   9. authority          -- a `superseded`/`withdrawn` ADR may not be cited from the
//                            documents that state current hard constraints.
//
// Usage:
//   node scripts/check-adr-lifecycle.mjs [--root <dir>] [--today YYYY-MM-DD]
//
// `--today` exists so the self-check can pin the calendar; a real run uses the local
// date. Overdue being a failure is deliberate: it is the only thing that makes a
// quarterly cadence actually happen.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const argv = process.argv.slice(2);
const rootFlag = argv.indexOf('--root');
const ROOT = rootFlag >= 0 ? path.resolve(argv[rootFlag + 1]) : path.resolve(HERE, '..');
const todayFlag = argv.indexOf('--today');

const DECISIONS = path.join(ROOT, 'docs', 'decisions');
const INDEX = path.join(DECISIONS, 'README.md');
const LEDGER = path.join(ROOT, 'scripts', 'adr-review-log.json');

const STATES = ['proposed', 'accepted', 'amended', 'superseded', 'withdrawn'];
const FILE_NAME = /^ADR-(\d{4})-[a-z0-9-]+\.md$/;
const KNOWN_EXT = ['.rs', '.ts', '.tsx', '.go', '.md', '.ps1', '.json', '.mjs', '.toml', '.drawio', '.yml', '.yaml', '.svg'];
const RESOLVE_ROOTS = ['', 'easyflowhub-app/', 'native/', 'scriptmgr-go/'];
const RENAMED_ROOTS = { 'scriptmgr/': 'scriptmgr-go/' };
const AUTHORITY_DOCS = ['docs/ARCHITECTURE_GUARDRAILS.md', 'docs/PROJECT_PROFILE.md', 'docs/roadmap.md'];

const failures = [];
const warn = [];
const fail = (msg) => failures.push(msg);

const pad = (n) => String(n).padStart(2, '0');
const pad4 = (n) => String(n).padStart(4, '0');

function today() {
  if (todayFlag >= 0) return argv[todayFlag + 1];
  const d = new Date();
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

function isDate(s) {
  if (typeof s !== 'string' || !/^\d{4}-\d{2}-\d{2}$/.test(s)) return false;
  const [y, m, d] = s.split('-').map(Number);
  const t = new Date(Date.UTC(y, m - 1, d));
  return t.getUTCFullYear() === y && t.getUTCMonth() === m - 1 && t.getUTCDate() === d;
}
const day = (s) => Date.UTC(...s.split('-').map((n, i) => (i === 1 ? Number(n) - 1 : Number(n)))) / 86400000;
const days = (a, b) => day(b) - day(a);

// ------------------------------------------------------------------ 1. collect files
if (!fs.existsSync(DECISIONS)) {
  console.error(`docs/decisions not found under ${ROOT}; is --root pointing at the repo?`);
  process.exit(1);
}

const adrs = [];
for (const entry of fs.readdirSync(DECISIONS, { withFileTypes: true })) {
  if (entry.isDirectory() || !entry.name.endsWith('.md') || entry.name === 'README.md') continue;
  const m = FILE_NAME.exec(entry.name);
  if (!m) {
    fail(`${entry.name}: not an ADR; decisions/ holds ADR-NNNN-<slug>.md and README.md only`);
    continue;
  }
  adrs.push({ id: m[1], file: entry.name, full: path.join(DECISIONS, entry.name) });
}
adrs.sort((a, b) => a.id.localeCompare(b.id));

// ------------------------------------------------------------------ 2. header fields
function header(text) {
  const fields = {};
  const lines = text.split('\n');
  for (let i = 1; i < lines.length; i += 1) {
    if (lines[i].startsWith('## ')) break;
    const m = /^-\s+([A-Za-z][A-Za-z -]*):\s*(.*)$/.exec(lines[i]);
    if (m) fields[m[1].trim().toLowerCase()] = m[2].trim();
  }
  return fields;
}

// `无（…）` and `none` mean "no relation"; anything else must name an ADR.
function refOf(value, label) {
  if (!value || /^(无|none|n\/a)$/i.test(value) || /^(无|none|n\/a)[（(\s]/i.test(value)) return null;
  const m = /ADR-(\d{4})/.exec(value);
  if (!m) {
    fail(`${label}: "${value.slice(0, 40)}" names no ADR`);
    return null;
  }
  return m[1];
}

for (const adr of adrs) {
  adr.text = fs.readFileSync(adr.full, 'utf8');
  const f = header(adr.text);
  adr.fields = {
    status: f.status || '',
    date: f.date || '',
    scope: f.scope || '',
    supersedes: f.supersedes,
    supersededBy: f['superseded-by'],
    amends: f.amends,
    amendedBy: f['amended-by'],
  };

  if (!STATES.includes(adr.fields.status)) {
    fail(`${adr.file}: status is "${adr.fields.status || '-'}", expected one of ${STATES.join(' / ')} -- token only, prose goes in the body`);
  }
  if (!isDate(adr.fields.date)) {
    fail(`${adr.file}: date is "${adr.fields.date || '-'}", expected a YYYY-MM-DD that exists`);
  }
  if (!adr.fields.scope) {
    fail(`${adr.file}: missing "scope:" -- it answers how far the decision reaches, which is what an expiry check reads`);
  }
}

// ------------------------------------------------------------------ 3. numbering
const byId = new Map(adrs.map((a) => [a.id, a]));
const nums = adrs.map((a) => Number(a.id)).sort((a, b) => a - b);
for (let i = 0; i < nums.length; i += 1) {
  if (nums[i] !== i + 1) {
    fail(`numbering: expected ADR-${pad4(i + 1)}, found ADR-${pad4(nums[i])} -- numbers stay contiguous and are never recycled`);
    break;
  }
}

// ------------------------------------------------------------------ 4. relations
function link(from, field, target, expect) {
  const where = (byId.get(from) || {}).file || `ADR-${from}`;
  const other = byId.get(target);
  if (!other) {
    fail(`${where}: ${field} points at ADR-${target}, which does not exist`);
    return;
  }
  const back = refOf(other.fields[expect], `ADR-${target} ${expect}`);
  if (back !== from) {
    fail(`${where}: ${field} ADR-${target}, but ADR-${target} does not record it back -- one-way pointers are how a replaced decision keeps getting cited`);
  }
}

for (const adr of adrs) {
  const f = adr.fields;
  const supersedes = refOf(f.supersedes, `${adr.file} supersedes`);
  const supersededBy = refOf(f.supersededBy, `${adr.file} superseded-by`);
  const amends = refOf(f.amends, `${adr.file} amends`);
  const amendedBy = refOf(f.amendedBy, `${adr.file} amended-by`);

  if (f.amends && !/§|#/.test(f.amends)) {
    fail(`${adr.file}: amends must name the section it rewrites (e.g. "ADR-0004 §2"); without it a partial amendment reads as a full replacement`);
  }
  if (supersedes) link(adr.id, 'supersedes', supersedes, 'supersededBy');
  if (supersededBy) link(adr.id, 'superseded-by', supersededBy, 'supersedes');
  if (amends) link(adr.id, 'amends', amends, 'amendedBy');
  if (amendedBy) link(adr.id, 'amended-by', amendedBy, 'amends');

  // ---------------------------------------------------------------- 5. state coherence
  const status = f.status;
  if (status === 'superseded' && !supersededBy) fail(`${adr.file}: status superseded but no superseded-by`);
  if (status === 'amended' && !amendedBy) fail(`${adr.file}: status amended but no amended-by`);
  if (supersededBy && status !== 'superseded') fail(`${adr.file}: has superseded-by but status is "${status}", expected superseded`);
  if (amendedBy && status !== 'amended') fail(`${adr.file}: has amended-by but status is "${status}", expected amended`);
  if ((status === 'accepted' || status === 'proposed' || status === 'withdrawn') && (supersededBy || amendedBy)) {
    fail(`${adr.file}: status ${status} cannot carry a replacement pointer`);
  }

  adr.supersededBy = supersededBy;
  adr.amendedBy = amendedBy;
}

// ------------------------------------------------------------------ 6. index
let indexed = new Map();
if (!fs.existsSync(INDEX)) {
  fail('docs/decisions/README.md: missing; it is the only entry point into this directory');
} else {
  const indexText = fs.readFileSync(INDEX, 'utf8');
  for (const line of indexText.split('\n')) {
    const m = /^\|\s*\[(\d{4})\]\(\.\/(ADR-\d{4}-[^)]+\.md)\)\s*\|(.*)$/.exec(line);
    if (!m) continue;
    const cells = m[3].split('|').map((c) => c.trim());
    const token = (cells.map((c) => /^`([a-z]+)`$/.exec(c)).find(Boolean) || [])[1] || '';
    const date = cells.map((c) => (isDate(c) ? c : '')).find(Boolean) || '';
    if (indexed.has(m[1])) fail(`README.md: ADR-${m[1]} listed twice`);
    indexed.set(m[1], { file: m[2], token, date });
  }
  for (const adr of adrs) {
    const row = indexed.get(adr.id);
    if (!row) {
      fail(`README.md: ADR-${adr.id} is not in the index -- an ADR nobody indexes is an ADR nobody re-reads`);
      continue;
    }
    if (row.file !== adr.file) fail(`README.md: ADR-${adr.id} links ${row.file}, the file is ${adr.file}`);
    if (row.token !== adr.fields.status) fail(`README.md: ADR-${adr.id} shows "${row.token || '-'}", the file says "${adr.fields.status}"`);
    if (row.date !== adr.fields.date) fail(`README.md: ADR-${adr.id} shows ${row.date || 'no date'}, the file says ${adr.fields.date}`);
  }
  for (const id of indexed.keys()) {
    if (!byId.has(id)) fail(`README.md: ADR-${id} is indexed but there is no such file`);
  }
}

// ------------------------------------------------------------------ 7. path resolution
function stripFences(text) {
  const out = [];
  let fenced = false;
  for (const line of text.split('\n')) {
    if (/^\s*(```|~~~)/.test(line)) {
      fenced = !fenced;
      continue;
    }
    if (!fenced) out.push(line);
  }
  return out.join('\n');
}

function looksLikePath(token) {
  if (KNOWN_EXT.some((ext) => token.endsWith(ext))) return true;
  return fs.existsSync(path.join(ROOT, token.split('/')[0]));
}

let pathsChecked = 0;
for (const adr of adrs) {
  for (const m of stripFences(adr.text).matchAll(/`([^`\n]+)`/g)) {
    const token = m[1].trim();
    if (!token.includes('/')) continue;
    if (/^[a-z][a-z0-9+.-]*:/i.test(token)) continue; // URL / scheme
    if (token.startsWith('/') || token.startsWith('~')) continue;
    if (/[*{}\s]/.test(token)) continue;

    const renamed = Object.keys(RENAMED_ROOTS).find((prefix) => token.startsWith(prefix));
    if (renamed) {
      fail(`${adr.file}: ${token} -- that directory is now ${RENAMED_ROOTS[renamed]}`);
      continue;
    }
    if (!looksLikePath(token)) continue;
    pathsChecked += 1;
    if (RESOLVE_ROOTS.some((prefix) => fs.existsSync(path.join(ROOT, prefix + token)))) continue;
    fail(`${adr.file}: ${token} does not resolve -- either the path moved (fix the text) or the decision no longer describes the code (amend it)`);
  }
}

// ------------------------------------------------------------------ 8. review ledger
let ledger = null;
if (!fs.existsSync(LEDGER)) {
  fail('scripts/adr-review-log.json: missing; a cadence with no ledger is a cadence nobody keeps');
} else {
  try {
    ledger = JSON.parse(fs.readFileSync(LEDGER, 'utf8'));
  } catch (error) {
    fail(`scripts/adr-review-log.json: ${error.message}`);
  }
}

let daysLeft = null;
if (ledger) {
  const cadence = ledger.cadenceDays;
  const deepAfter = ledger.deepAfterOverdueDays;
  if (!Number.isInteger(cadence) || cadence < 1 || cadence > 365) fail('adr-review-log: cadenceDays must be an integer between 1 and 365');
  if (!Number.isInteger(deepAfter) || deepAfter < 0) fail('adr-review-log: deepAfterOverdueDays must be a non-negative integer');

  const reviews = Array.isArray(ledger.reviews) ? ledger.reviews.slice() : [];
  if (!reviews.length) fail('adr-review-log: reviews is empty');
  const stamp = today();
  if (!isDate(stamp)) fail(`--today "${stamp}" is not a date`);

  let prev = null;
  for (const r of reviews) {
    const at = `${r.date} (${r.kind})`;
    if (!isDate(r.date)) fail(`adr-review-log: ${at}: date is not a YYYY-MM-DD that exists`);
    if (!['sweep', 'deep'].includes(r.kind)) fail(`adr-review-log: ${at}: kind must be sweep or deep`);
    if (!isDate(r.next)) fail(`adr-review-log: ${at}: next is not a date`);
    if (isDate(r.date) && isDate(r.next) && !(day(r.next) > day(r.date))) fail(`adr-review-log: ${at}: next (${r.next}) is not after the review date`);
    if (isDate(r.date) && isDate(stamp) && day(r.date) > day(stamp)) fail(`adr-review-log: ${at}: the review is dated in the future`);
    if (isDate(r.date) && isDate(r.next) && Number.isInteger(cadence) && days(r.date, r.next) > cadence) {
      fail(`adr-review-log: ${at}: next is ${days(r.date, r.next)} days out, beyond the ${cadence}-day cadence`);
    }
    if (typeof r.summary !== 'string' || r.summary.trim().length < 8) fail(`adr-review-log: ${at}: summary is empty; a review with no finding is a review that did not happen`);

    if (prev && isDate(prev.next) && isDate(r.date)) {
      const late = days(prev.next, r.date);
      if (late > deepAfter && r.kind !== 'deep') {
        fail(`adr-review-log: ${at}: started ${late} days after the previous due date, so it had to be deep (limit ${deepAfter})`);
      }
    }
    prev = r;
  }

  const last = reviews[reviews.length - 1];
  if (last && isDate(last.next) && isDate(stamp)) {
    daysLeft = days(stamp, last.next);
    if (daysLeft < 0) {
      fail(`adr-review-log: review overdue by ${-daysLeft} days (due ${last.next}); run the sweep, then record it with a new "next"`);
    } else if (daysLeft <= 14) {
      warn.push(`review due in ${daysLeft} day(s) (${last.next}) -- schedule the sweep`);
    }
  }
}

// ------------------------------------------------------------------ 9. authority docs
const dead = adrs.filter((a) => a.fields.status === 'superseded' || a.fields.status === 'withdrawn').map((a) => a.id);
if (dead.length) {
  for (const doc of AUTHORITY_DOCS) {
    const full = path.join(ROOT, doc);
    if (!fs.existsSync(full)) continue;
    for (const m of fs.readFileSync(full, 'utf8').matchAll(/ADR-(\d{4})/g)) {
      if (dead.includes(m[1])) fail(`${doc}: cites ADR-${m[1]} as a current constraint, but that ADR is no longer in force`);
    }
  }
}

// ------------------------------------------------------------------ report
if (failures.length) {
  console.error('ADR lifecycle guard failed:\n');
  for (const f of failures) console.error(`  FAIL ${f}`);
  console.error(`\n${failures.length} problem(s). Rules: docs/decisions/README.md`);
  process.exit(1);
}

const tally = {};
for (const adr of adrs) tally[adr.fields.status] = (tally[adr.fields.status] || 0) + 1;
console.log(
  `ADR lifecycle: ${adrs.length} decisions (${Object.entries(tally).map(([k, v]) => `${v} ${k}`).join(', ')}), ` +
    `${indexed.size} indexed, ${pathsChecked} path references resolved`,
);
if (daysLeft !== null) {
  const last = ledger.reviews[ledger.reviews.length - 1];
  console.log(`  review log: last ${last.date} (${last.kind}), next ${last.next}, ${daysLeft} day(s) left`);
}
for (const w of warn) console.log(`  note: ${w}`);
