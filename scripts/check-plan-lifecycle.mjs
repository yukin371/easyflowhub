#!/usr/bin/env node
// Plan lifecycle guard.
//
// `docs/plans` used to be one flat directory of 41 markdown files with no rule about
// which of them still mattered: a finished plan, an abandoned one and the design
// currently being implemented all looked identical, and the only signal was a
// `状态:` line inside each file that nobody was obliged to update. That line is
// exactly the kind of field that never gets updated, so the state is expressed by
// **placement** instead, and this guard keeps placement and the declared token in
// agreement:
//
//   1. layout      -- docs/plans/ holds README.md plus exactly the five state dirs.
//                     A loose file in the root has no state, which is the old bug.
//   2. state line  -- exactly one `> 状态：`token`` in the header block, token from
//                     the fixed vocabulary. Two lines means two sources of truth.
//   3. placement   -- folder == map(token). This is the rule that makes "move it"
//                     the only way to change state.
//   4. naming      -- live buckets carry a date prefix; archive is exempt because
//                     frozen files should not be renamed for cosmetics.
//   5. index       -- every plan is linked from docs/plans/README.md. An unindexed
//                     plan is a plan nobody will find.
//   6. supersede   -- `superseded` must name a successor that exists; otherwise
//                     "superseded" is a word, not a route.
//   7. references  -- any `docs/plans/...md` path mentioned anywhere in the repo
//                     (script comments, JSON pins, prose) must resolve. Moving a
//                     file is how these rot, and they are invisible to the
//                     markdown-link check in check-doc-budget.mjs.
//
// Usage: node scripts/check-plan-lifecycle.mjs [--root <dir>]

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const argv = process.argv.slice(2);
const rootFlag = argv.indexOf('--root');
const ROOT = rootFlag >= 0 ? path.resolve(argv[rootFlag + 1]) : path.resolve(HERE, '..');

const PLANS = path.join(ROOT, 'docs', 'plans');
const INDEX = path.join(PLANS, 'README.md');

/** folder -> accepted tokens. `archive` is where both frozen and replaced plans go. */
const STATES = {
  proposals: ['proposed'],
  active: ['active'],
  completed: ['completed'],
  rejected: ['rejected'],
  archive: ['archived', 'superseded'],
};
const FOLDER_OF = new Map();
for (const [folder, tokens] of Object.entries(STATES)) {
  for (const token of tokens) FOLDER_OF.set(token, folder);
}
/** Buckets where a date prefix is required; archive keeps historical names. */
const DATED = new Set(['proposals', 'active', 'completed', 'rejected']);
const HEADER_LINES = 20;

const rel = (file) => path.relative(ROOT, file).split(path.sep).join('/');
const key = (file) => path.resolve(file).toLowerCase();

function walk(dir, out = []) {
  if (!fs.existsSync(dir)) return out;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, out);
    else if (entry.name.endsWith('.md')) out.push(full);
  }
  return out;
}

const failures = [];
const fail = (line) => failures.push(line);

// --------------------------------------------------------------- 1. layout
if (!fs.existsSync(PLANS)) {
  console.error(`FAIL plan lifecycle: ${rel(PLANS)} does not exist`);
  process.exit(1);
}
const loose = [];
for (const entry of fs.readdirSync(PLANS, { withFileTypes: true })) {
  if (entry.isDirectory()) {
    if (!(entry.name in STATES)) {
      fail(`${rel(path.join(PLANS, entry.name))}/: not a lifecycle state (expected ${Object.keys(STATES).join(' / ')})`);
    }
    continue;
  }
  if (entry.name !== 'README.md') loose.push(entry.name);
}
if (loose.length > 0) {
  fail(
    `${rel(PLANS)}: ${loose.length} file(s) in the root with no state -- move each into ${Object.keys(STATES).join('/')} (${loose.slice(0, 3).join(', ')}${loose.length > 3 ? ', ...' : ''})`,
  );
}

// --------------------------------------------------------------- 2-4. per file
const STATUS = /^[\s>*_#-]*(?:\*\*)?(?:状态|status)(?:\*\*)?\s*[:：]/i;
const plans = [];
for (const folder of Object.keys(STATES)) {
  const dir = path.join(PLANS, folder);
  // A missing bucket is not a problem: git cannot track an empty directory, so the
  // bucket legitimately disappears once its last plan moves on.
  if (!fs.existsSync(dir)) continue;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (entry.isDirectory()) {
      fail(`${rel(path.join(dir, entry.name))}/: nested directories are not a lifecycle state`);
      continue;
    }
    if (!entry.name.endsWith('.md')) continue;
    plans.push({ folder, file: path.join(dir, entry.name), name: entry.name });
  }
}

const statusOf = new Map();
for (const plan of plans) {
  const name = rel(plan.file);
  const lines = fs.readFileSync(plan.file, 'utf8').split('\n').slice(0, HEADER_LINES);
  const hits = lines.map((line, i) => (STATUS.test(line) ? i : -1)).filter((i) => i >= 0);

  if (hits.length === 0) {
    fail(`${name}: no \`> 状态：\`token\`\` in the first ${HEADER_LINES} lines -- the file's state is otherwise unknowable`);
    continue;
  }
  if (hits.length > 1) {
    fail(`${name}: ${hits.length} status lines (lines ${hits.map((i) => i + 1).join(', ')}) -- keep exactly one, the folder already says the rest`);
    continue;
  }

  const value = lines[hits[0]].replace(STATUS, '').trim();
  const backticked = value.match(/^`([a-z-]+)`/);
  const bare = value.match(/^([a-z-]+)/);
  const token = backticked ? backticked[1] : bare ? bare[1] : null;
  if (token === null) {
    fail(`${name}: status must start with a canonical token in backticks, got "${value.slice(0, 40)}"`);
    continue;
  }
  if (!FOLDER_OF.has(token)) {
    fail(`${name}: unknown status token "${token}" (expected ${[...FOLDER_OF.keys()].join(' / ')})`);
    continue;
  }
  statusOf.set(plan.file, token);

  const expected = FOLDER_OF.get(token);
  if (expected !== plan.folder) {
    fail(`${name}: status is "${token}" (${expected}/) but the file sits in ${plan.folder}/ -- move it, do not relabel it`);
  }
  if (DATED.has(plan.folder) && !/^\d{4}-\d{2}-\d{2}-[a-z0-9.-]+\.md$/.test(plan.name)) {
    fail(`${name}: ${plan.folder}/ requires YYYY-MM-DD-slug.md; archive/ is the only place for legacy names`);
  }
  if (token === 'superseded') {
    const header = lines.join('\n');
    if (!/(superseded_by|replacement|取代者|取代为)/i.test(header)) {
      fail(`${name}: superseded without a successor -- add \`> superseded_by:\` pointing at the plan that replaced it`);
    } else {
      const mentions = [...header.matchAll(/`([^`]+\.md)`|\]\(([^)\s]+\.md)/g)]
        .map((m) => m[1] || m[2])
        .filter(Boolean);
      const resolves = mentions.some((mention) => {
        const asLink = path.resolve(path.dirname(plan.file), mention);
        const asRepo = path.resolve(ROOT, mention);
        return fs.existsSync(asLink) || fs.existsSync(asRepo);
      });
      if (!resolves) {
        fail(`${name}: superseded_by names nothing that exists (${mentions.join(', ') || 'no path at all'})`);
      }
    }
  }
}

// --------------------------------------------------------------- 5. index
if (!fs.existsSync(INDEX)) {
  fail(`${rel(INDEX)}: missing -- it is the only index of the lifecycle`);
} else {
  const text = fs.readFileSync(INDEX, 'utf8');
  const linked = new Set();
  for (const match of text.matchAll(/\]\(([^)\s]+)\)/g)) {
    const raw = match[1];
    if (raw.startsWith('#') || /^[a-z][a-z0-9+.-]*:/i.test(raw)) continue;
    linked.add(key(path.resolve(path.dirname(INDEX), raw.split('#')[0])));
  }
  for (const plan of plans) {
    if (!linked.has(key(plan.file))) {
      fail(`${rel(plan.file)}: not linked from ${rel(INDEX)} -- an unindexed plan cannot be found again`);
    }
  }
}

// --------------------------------------------------------------- 6. references
const TEXT = new Set(['.md', '.mjs', '.json', '.ps1', '.py', '.rs', '.ts', '.tsx', '.yml', '.yaml', '.toml']);
const SKIP = new Set(['node_modules', 'target', '.git', 'dist', 'build', '.workbuddy']);
const SELF = new Set(['check-plan-lifecycle.mjs', 'check-plan-lifecycle.selfcheck.mjs']);
// Repo-relative mentions only, restricted to the characters plan file names use: a
// looser class glues `](docs/plans/x.md)` onto the path and reports a phantom failure.
const PLAN_PATH = /docs\/plans\/[A-Za-z0-9._/-]*\.md/g;

function textFiles(dir, out = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (entry.isDirectory()) {
      if (!SKIP.has(entry.name)) textFiles(path.join(dir, entry.name), out);
      continue;
    }
    if (SELF.has(entry.name)) continue;
    if (TEXT.has(path.extname(entry.name).toLowerCase())) out.push(path.join(dir, entry.name));
  }
  return out;
}

let refCount = 0;
for (const file of textFiles(ROOT)) {
  const raw = fs.readFileSync(file, 'utf8');
  // A fenced block is a command or a template (`git mv ... 2026-11-25-xxx.md`), not a
  // claim that the path exists -- judge prose only.
  const text = file.endsWith('.md') ? raw.replace(/```[\s\S]*?```/g, '') : raw;
  for (const match of text.matchAll(PLAN_PATH)) {
    const mentioned = match[0];
    // A glob, an ellipsis or a template in prose is not a claim that the file exists.
    if (mentioned.includes('*') || mentioned.includes('...') || mentioned.includes('${')) continue;
    refCount += 1;
    if (!fs.existsSync(path.resolve(ROOT, mentioned))) {
      fail(`${rel(file)}: stale reference -> ${mentioned}`);
    }
  }
}

// --------------------------------------------------------------- report
if (failures.length > 0) {
  for (const line of failures) console.error(`FAIL ${line}`);
  console.error(`Plan lifecycle guard failed (${failures.length} problem(s), ${plans.length} plans)`);
  process.exit(1);
}
const counts = Object.keys(STATES)
  .map((folder) => `${folder} ${plans.filter((p) => p.folder === folder).length}`)
  .join(', ');
console.log(`ok layout: ok (${plans.length} plans in five buckets: ${counts})`);
console.log('ok state lines: ok (one canonical token per file, placement agrees)');
console.log(`ok index: ok (every plan linked from ${rel(INDEX)})`);
console.log(`ok references: ok (${refCount} path mentions resolve)`);
console.log(`Plan lifecycle guard passed (${plans.length} plans).`);
