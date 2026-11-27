// Self-check for the doc budget guard.
//
// A gate that has never been seen to fail is not evidence of anything: the real tree
// is well-formed, so every rule in the guard could be dead code and it would still
// print PASS. Each case below builds a throwaway repo in the OS temp directory,
// breaks exactly one rule, and asserts the specific violation comes back -- plus the
// "must not fire" cases, because a guard that also rejects legal layouts collects
// exemptions until it means nothing.
//
// Run: node --test scripts/check-doc-budget.selfcheck.mjs

import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const GUARD = path.join(HERE, 'check-doc-budget.mjs');

const BUDGET = {
  softBytes: 1024,
  hardBytes: 4096,
  pinnedBytes: {},
};

/** A tiny repo that satisfies every rule. */
function baseline() {
  return {
    'scripts/doc-budget.json': `${JSON.stringify(BUDGET, null, 2)}\n`,
    'README.md': '# readme\n',
    'docs/README.md': '# docs\n\n- [route](roadmap.md)\n',
    'docs/roadmap.md': [
      '# Roadmap',
      '',
      '### Recent Progress',
      '',
      '- [2026-11](./roadmap/progress-2026-11.md)',
      '',
    ].join('\n'),
    'docs/roadmap/progress-2026-11.md': '# 2026-11\n\n- `2026-11-27` did a thing\n',
  };
}

function makeRepo(files = baseline()) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'doc-budget-'));
  for (const [name, text] of Object.entries(files)) {
    const target = path.join(root, name);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, text, 'utf8');
  }
  return root;
}

function run(root, ...args) {
  const result = spawnSync(process.execPath, [GUARD, '--root', root, ...args], { encoding: 'utf8' });
  return { code: result.status, out: result.stdout, err: result.stderr };
}

test('a well-formed tree passes', () => {
  const { code, out } = run(makeRepo());
  assert.equal(code, 0, out);
  assert.match(out, /Doc budget guard passed/);
});

test('a file over the hard cap fails even when pinned', () => {
  const files = baseline();
  files['docs/roadmap.md'] += `\n${'x'.repeat(BUDGET.hardBytes + 1)}\n`;
  const { code, err } = run(makeRepo(files));
  assert.equal(code, 1);
  assert.match(err, /docs\/roadmap\.md: .* over the hard cap/);
});

test('a file over the soft cap must be pinned', () => {
  const files = baseline();
  files['docs/big.md'] = `${'x'.repeat(BUDGET.softBytes + 1)}\n`;
  const { code, err } = run(makeRepo(files));
  assert.equal(code, 1);
  assert.match(err, /docs\/big\.md: .* not pinned/);
});

test('a pinned file may not grow', () => {
  const files = baseline();
  files['scripts/doc-budget.json'] = `${JSON.stringify(
    { ...BUDGET, pinnedBytes: { 'docs/big.md': BUDGET.softBytes + 1 } },
    null,
    2,
  )}\n`;
  files['docs/big.md'] = `${'x'.repeat(BUDGET.softBytes + 200)}\n`;
  const { code, err } = run(makeRepo(files));
  assert.equal(code, 1);
  assert.match(err, /docs\/big\.md: grew .* closed for growth/);
});

test('--pin lowers a pin but refuses to raise one', () => {
  const files = baseline();
  files['scripts/doc-budget.json'] = `${JSON.stringify(
    { ...BUDGET, pinnedBytes: { 'docs/big.md': BUDGET.softBytes + 500 } },
    null,
    2,
  )}\n`;
  files['docs/big.md'] = `${'x'.repeat(BUDGET.softBytes + 100)}\n`;
  const root = makeRepo(files);

  const tightened = run(root, '--pin');
  assert.equal(tightened.code, 0, tightened.err);
  const written = JSON.parse(fs.readFileSync(path.join(root, 'scripts/doc-budget.json'), 'utf8'));
  const actual = fs.statSync(path.join(root, 'docs/big.md')).size;
  assert.equal(written.pinnedBytes['docs/big.md'], actual);

  // Growing the file and pinning again must be refused, which is what stops `--pin`
  // from being a way to launder growth.
  fs.writeFileSync(path.join(root, 'docs/big.md'), `${'x'.repeat(BUDGET.softBytes + 400)}\n`, 'utf8');
  const raised = run(root, '--pin');
  assert.equal(raised.code, 1);
  assert.match(raised.err, /cannot raise a pinned size/);
});

test('a broken relative link fails and names the file', () => {
  const files = baseline();
  files['docs/README.md'] = '# docs\n\n- [gone](./plans/missing.md)\n';
  const { code, err } = run(makeRepo(files));
  assert.equal(code, 1);
  assert.match(err, /docs\/README\.md: link does not resolve -> \.\/plans\/missing\.md/);
});

test('external links, anchors and scheme links are not judged', () => {
  const files = baseline();
  files['docs/README.md'] = [
    '# docs',
    '',
    '- [web](https://example.com/x)',
    '- [anchor](#top)',
    '- [asset](asset:thing)',
    '- [mail](mailto:a@b.c)',
    '',
  ].join('\n');
  const { code, out } = run(makeRepo(files));
  assert.equal(code, 0, out);
});

test('a dated entry in the route document fails', () => {
  const files = baseline();
  files['docs/roadmap.md'] += "\n- `2026-11-27` appended history to the route doc\n";
  const { code, err } = run(makeRepo(files));
  assert.equal(code, 1);
  assert.match(err, /docs\/roadmap\.md: 1 dated progress entries/);
});

test('a progress file the index does not link is an orphan', () => {
  const files = baseline();
  files['docs/roadmap/progress-2026-10.md'] = '# 2026-10\n\n- `2026-10-01` old\n';
  const { code, err } = run(makeRepo(files));
  assert.equal(code, 1);
  assert.match(err, /progress-2026-10\.md: not linked from docs\/roadmap\.md/);
});

test('a route layer with an unrecognised name is rejected', () => {
  const files = baseline();
  files['docs/roadmap/notes.md'] = '# misc\n';
  files['docs/roadmap.md'] += '\n- [notes](./roadmap/notes.md)\n';
  const { code, err } = run(makeRepo(files));
  assert.equal(code, 1);
  assert.match(err, /notes\.md: route layers must be progress-YYYY-MM\.md or vX\.Y\.Z\.md/);
});

test('a link inside a code span is documentation, not a link', () => {
  // The roadmap entry that explains the `](./x)` -> `](../x)` rewrite while the split
  // was happening is exactly this shape; judging it would fail on prose.
  const files = baseline();
  files['docs/README.md'] = '# docs\n\nRewrote `](./x)` into `](../x)` in 22 places.\n';
  const { code, out } = run(makeRepo(files));
  assert.equal(code, 0, out);
});

test('a missing budget file says how to create it', () => {
  const files = baseline();
  delete files['scripts/doc-budget.json'];
  const { code, err } = run(makeRepo(files));
  assert.equal(code, 1);
  assert.match(err, /doc-budget\.json is missing; create it with --pin/);
});
