// Self-check for the plan lifecycle guard.
//
// Same argument as the other guards' self-checks: the real tree is well-formed, so a
// rule could be dead code and the guard would still print PASS. Each case below builds
// a throwaway tree, breaks exactly one rule, and asserts the specific message comes
// back -- plus the "must not fire" cases, because a guard that also rejects legal
// layouts collects exemptions until it means nothing.
//
// Run: node --test scripts/check-plan-lifecycle.selfcheck.mjs

import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const GUARD = path.join(HERE, 'check-plan-lifecycle.mjs');

/** A tree that satisfies every rule. */
function baseline() {
  return {
    'docs/plans/README.md': [
      '# Plans',
      '',
      '- [a](./active/2026-01-01-a.md)',
      '- [b](./archive/legacy.md)',
      '',
    ].join('\n'),
    'docs/plans/active/2026-01-01-a.md': '# A\n\n> 状态：`active`\n',
    'docs/plans/archive/legacy.md': '# Legacy\n\n> 状态：`archived`（frozen）\n',
  };
}

function run(files) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'plan-lifecycle-'));
  for (const [name, body] of Object.entries(files)) {
    const full = path.join(root, name);
    fs.mkdirSync(path.dirname(full), { recursive: true });
    fs.writeFileSync(full, body, 'utf8');
  }
  const result = spawnSync(process.execPath, [GUARD, '--root', root], { encoding: 'utf8' });
  fs.rmSync(root, { recursive: true, force: true });
  return { code: result.status, out: `${result.stdout}${result.stderr}` };
}

test('a well-formed tree passes', () => {
  const { code, out } = run(baseline());
  assert.equal(code, 0, out);
  assert.match(out, /Plan lifecycle guard passed/);
});

test('a plan left in the root has no state', () => {
  const files = baseline();
  files['docs/plans/2026-01-01-loose.md'] = '# Loose\n\n> 状态：`active`\n';
  const { code, out } = run(files);
  assert.equal(code, 1);
  assert.match(out, /file\(s\) in the root with no state/);
});

test('a stray directory is not a lifecycle state', () => {
  const files = baseline();
  files['docs/plans/wip/2026-01-01-x.md'] = '# X\n\n> 状态：`active`\n';
  const { code, out } = run(files);
  assert.equal(code, 1);
  assert.match(out, /not a lifecycle state/);
});

test('a plan without a status line is rejected', () => {
  const files = baseline();
  files['docs/plans/active/2026-01-01-a.md'] = '# A\n\nno status here\n';
  const { code, out } = run(files);
  assert.equal(code, 1);
  assert.match(out, /no `> 状态：`token``/);
});

test('two status lines are two sources of truth', () => {
  const files = baseline();
  files['docs/plans/active/2026-01-01-a.md'] = '# A\n\n> 状态：`active`\n\n- status: accepted\n';
  const { code, out } = run(files);
  assert.equal(code, 1);
  assert.match(out, /2 status lines/);
});

test('an invented token is rejected', () => {
  const files = baseline();
  files['docs/plans/active/2026-01-01-a.md'] = '# A\n\n> 状态：`in-progress`\n';
  const { code, out } = run(files);
  assert.equal(code, 1);
  assert.match(out, /unknown status token "in-progress"/);
});

test('placing an active plan in completed/ fails rather than passing quietly', () => {
  const files = baseline();
  files['docs/plans/completed/2026-01-01-a.md'] = files['docs/plans/active/2026-01-01-a.md'];
  delete files['docs/plans/active/2026-01-01-a.md'];
  files['docs/plans/README.md'] = '# Plans\n\n- [a](./completed/2026-01-01-a.md)\n';
  const { code, out } = run(files);
  assert.equal(code, 1);
  assert.match(out, /move it, do not relabel it/);
});

test('live buckets require a date prefix', () => {
  const files = baseline();
  files['docs/plans/active/a.md'] = '# A\n\n> 状态：`active`\n';
  delete files['docs/plans/active/2026-01-01-a.md'];
  files['docs/plans/README.md'] = '# Plans\n\n- [a](./active/a.md)\n';
  const { code, out } = run(files);
  assert.equal(code, 1);
  assert.match(out, /requires YYYY-MM-DD-slug\.md/);
});

test('archive keeps legacy names (must not fire)', () => {
  const { code, out } = run(baseline());
  assert.equal(code, 0, out);
});

test('an unindexed plan is reported', () => {
  const files = baseline();
  files['docs/plans/active/2026-01-02-orphan.md'] = '# Orphan\n\n> 状态：`active`\n';
  const { code, out } = run(files);
  assert.equal(code, 1);
  assert.match(out, /not linked from docs\/plans\/README\.md/);
});

test('superseded without a successor is a word, not a route', () => {
  const files = baseline();
  files['docs/plans/archive/legacy.md'] = '# Legacy\n\n> 状态：`superseded`\n';
  const { code, out } = run(files);
  assert.equal(code, 1);
  assert.match(out, /superseded without a successor/);
});

test('superseded pointing at a file that does not exist is rejected', () => {
  const files = baseline();
  files['docs/plans/archive/legacy.md'] =
    '# Legacy\n\n> 状态：`superseded`\n> superseded_by: `docs/plans/active/2026-01-09-gone.md`\n';
  const { code, out } = run(files);
  assert.equal(code, 1);
  assert.match(out, /names nothing that exists/);
});

test('superseded with a resolvable successor passes (must not fire)', () => {
  const files = baseline();
  files['docs/plans/archive/legacy.md'] =
    '# Legacy\n\n> 状态：`superseded`\n> superseded_by: `docs/plans/active/2026-01-01-a.md`\n';
  const { code, out } = run(files);
  assert.equal(code, 0, out);
});

test('a stale path in a script comment is caught', () => {
  const files = baseline();
  files['scripts/probe.mjs'] = '// see docs/plans/active/2026-01-09-gone.md\n';
  const { code, out } = run(files);
  assert.equal(code, 1);
  assert.match(out, /stale reference -> docs\/plans\/active\/2026-01-09-gone\.md/);
});

test('a path inside a fenced block is a template, not a claim (must not fire)', () => {
  const files = baseline();
  files['docs/plans/README.md'] += '\n```bash\ngit mv a.md active/2026-01-01-xxx.md\n```\n';
  const { code, out } = run(files);
  assert.equal(code, 0, out);
});
