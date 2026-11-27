// Self-check for the ADR lifecycle guard.
//
// Same reasoning as the other guards' self-checks: the real tree is well-formed, so
// every rule here could be dead code and the guard would still print OK. Each case
// builds a throwaway repo with one rule broken and asserts the specific message comes
// back, plus the "must not fire" cases -- a guard that also rejects legal layouts
// collects exemptions until it means nothing.
//
// The review-ledger cases pin the calendar with --today, otherwise they would rot:
// a fixture written today is overdue in three months.
//
// Run: node --test scripts/check-adr-lifecycle.selfcheck.mjs

import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const GUARD = path.join(HERE, 'check-adr-lifecycle.mjs');
const TODAY = '2026-11-27';

function adr(id, slug, fields, body = '## Context\n\nNothing yet.\n') {
  const header = Object.entries(fields).map(([k, v]) => `- ${k}: ${v}`).join('\n');
  return `# ADR-${id} ${slug}\n\n${header}\n\n${body}`;
}

function row(id, slug, token, date = '2026-01-01', scope = '长期') {
  return `| [${id}](./ADR-${id}-${slug}.md) | ${slug} | \`${token}\` | ${date} | ${scope} |`;
}

function index(...rows) {
  return ['# Decisions', '', '| ADR | 决策 | 状态 | 日期 | scope |', '|---|---|---|---|---|', ...rows, ''].join('\n');
}

function ledger(reviews) {
  return `${JSON.stringify({ cadenceDays: 90, deepAfterOverdueDays: 30, reviews }, null, 2)}\n`;
}

const REVIEW = { date: '2026-11-01', kind: 'sweep', next: '2027-01-30', summary: 'routine sweep, all decisions still match the code' };

function baseline() {
  const fields = { status: 'accepted', date: '2026-01-01', scope: '长期' };
  return {
    'docs/decisions/README.md': index(row('0001', 'alpha', 'accepted')),
    'docs/decisions/ADR-0001-alpha.md': adr('0001', 'alpha', fields),
    'scripts/adr-review-log.json': ledger([REVIEW]),
  };
}

function makeRepo(files = baseline()) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'adr-lifecycle-'));
  for (const [name, text] of Object.entries(files)) {
    const target = path.join(root, name);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, text, 'utf8');
  }
  return root;
}

function run(root, ...args) {
  const result = spawnSync(process.execPath, [GUARD, '--root', root, '--today', TODAY, ...args], { encoding: 'utf8' });
  return { code: result.status, out: result.stdout, err: result.stderr };
}

test('a well-formed tree passes and reports the calendar', () => {
  const { code, out } = run(makeRepo());
  assert.equal(code, 0, out);
  assert.match(out, /ADR lifecycle: 1 decisions \(1 accepted\), 1 indexed/);
  assert.match(out, /next 2027-01-30/);
});

test('a loose markdown file in decisions/ is rejected', () => {
  const files = baseline();
  files['docs/decisions/notes.md'] = '# scratch\n';
  const { code, err } = run(makeRepo(files));
  assert.equal(code, 1);
  assert.match(err, /notes\.md: not an ADR/);
});

test('a free-text status is rejected', () => {
  const files = baseline();
  files['docs/decisions/ADR-0001-alpha.md'] = adr('0001', 'alpha', {
    status: 'accepted（2026-01-01 确认）',
    date: '2026-01-01',
    scope: '长期',
  });
  const { code, err } = run(makeRepo(files));
  assert.equal(code, 1);
  assert.match(err, /status is "accepted（2026-01-01 确认）", expected one of/);
  assert.match(err, /token only/);
});

test('scope is required, because an expiry check reads it', () => {
  const files = baseline();
  files['docs/decisions/ADR-0001-alpha.md'] = adr('0001', 'alpha', { status: 'accepted', date: '2026-01-01' });
  const { code, err } = run(makeRepo(files));
  assert.equal(code, 1);
  assert.match(err, /missing "scope:"/);
});

test('a gap in the numbering is rejected', () => {
  const files = baseline();
  files['docs/decisions/ADR-0003-gamma.md'] = adr('0003', 'gamma', { status: 'accepted', date: '2026-01-01', scope: '长期' });
  files['docs/decisions/README.md'] = index(row('0001', 'alpha', 'accepted'), row('0003', 'gamma', 'accepted'));
  const { code, err } = run(makeRepo(files));
  assert.equal(code, 1);
  assert.match(err, /expected ADR-0002, found ADR-0003/);
});

test('a one-way supersedes pointer is rejected', () => {
  const files = baseline();
  files['docs/decisions/ADR-0002-beta.md'] = adr('0002', 'beta', {
    status: 'accepted',
    date: '2026-02-01',
    scope: '长期',
    supersedes: 'ADR-0001',
  });
  files['docs/decisions/README.md'] = index(row('0001', 'alpha', 'accepted'), row('0002', 'beta', 'accepted'));
  const { code, err } = run(makeRepo(files));
  assert.equal(code, 1);
  assert.match(err, /one-way pointers are how a replaced decision keeps getting cited/);
});

test('amends without a section number is rejected', () => {
  const files = baseline();
  files['docs/decisions/ADR-0002-beta.md'] = adr('0002', 'beta', {
    status: 'accepted',
    date: '2026-02-01',
    scope: '长期',
    amends: 'ADR-0001',
  });
  files['docs/decisions/README.md'] = index(row('0001', 'alpha', 'accepted'), row('0002', 'beta', 'accepted'));
  const { code, err } = run(makeRepo(files));
  assert.equal(code, 1);
  assert.match(err, /amends must name the section it rewrites/);
});

test('superseded without a successor is rejected', () => {
  const files = baseline();
  files['docs/decisions/ADR-0001-alpha.md'] = adr('0001', 'alpha', { status: 'superseded', date: '2026-01-01', scope: '长期' });
  const { code, err } = run(makeRepo(files));
  assert.equal(code, 1);
  assert.match(err, /status superseded but no superseded-by/);
});

test('an ADR missing from the index is rejected', () => {
  const files = baseline();
  files['docs/decisions/README.md'] = '# Decisions\n\nNo table here.\n';
  const { code, err } = run(makeRepo(files));
  assert.equal(code, 1);
  assert.match(err, /ADR-0001 is not in the index/);
});

test('an index row whose token disagrees with the file is rejected', () => {
  const files = baseline();
  files['docs/decisions/README.md'] = index(row('0001', 'alpha', 'proposed'));
  const { code, err } = run(makeRepo(files));
  assert.equal(code, 1);
  assert.match(err, /shows "proposed", the file says "accepted"/);
});

test('a renamed module directory is named as such', () => {
  const files = baseline();
  files['docs/decisions/ADR-0001-alpha.md'] = adr(
    '0001',
    'alpha',
    { status: 'accepted', date: '2026-01-01', scope: '长期' },
    '## Decision\n\nThe policy lives in `scriptmgr/internal/localapi`.\n',
  );
  const { code, err } = run(makeRepo(files));
  assert.equal(code, 1);
  assert.match(err, /that directory is now scriptmgr-go\//);
});

test('a path that no longer resolves is rejected', () => {
  const files = baseline();
  files['docs/decisions/ADR-0001-alpha.md'] = adr(
    '0001',
    'alpha',
    { status: 'accepted', date: '2026-01-01', scope: '长期' },
    '## Decision\n\nOwner is `docs/decisions/gone.md`.\n',
  );
  const { code, err } = run(makeRepo(files));
  assert.equal(code, 1);
  assert.match(err, /does not resolve/);
});

test('bare filenames, mime types, CIDRs and endpoints are not treated as paths', () => {
  const files = baseline();
  files['docs/decisions/ADR-0001-alpha.md'] = adr(
    '0001',
    'alpha',
    { status: 'accepted', date: '2026-01-01', scope: '长期' },
    [
      '## Decision',
      '',
      '- the owner is `voice.rs`, with `audio/wav` as the mime type',
      '- bind only `127.0.0.0/8` or `::1`',
      '- relay accepts `POST /v1/audio/transcriptions` at `http://127.0.0.1:8787`',
      '',
    ].join('\n'),
  );
  const { code, out } = run(makeRepo(files));
  assert.equal(code, 0, out);
});

test('"supersedes: 无" means no relation, not a dangling reference', () => {
  const files = baseline();
  files['docs/decisions/ADR-0001-alpha.md'] = adr('0001', 'alpha', {
    status: 'accepted',
    date: '2026-01-01',
    scope: '长期',
    supersedes: '无（与 ADR-0009 的方向兼容）',
  });
  const { code, out } = run(makeRepo(files));
  assert.equal(code, 0, out);
});

test('an overdue review fails', () => {
  const files = baseline();
  files['scripts/adr-review-log.json'] = ledger([{ ...REVIEW, next: '2026-11-20' }]);
  const { code, err } = run(makeRepo(files));
  assert.equal(code, 1);
  assert.match(err, /review overdue by 7 days/);
});

test('a review that starts late must be deep', () => {
  const files = baseline();
  files['scripts/adr-review-log.json'] = ledger([
    { date: '2026-01-01', kind: 'sweep', next: '2026-02-01', summary: 'first sweep, nothing changed' },
    { date: '2026-06-01', kind: 'sweep', next: '2026-08-30', summary: 'late but only a sweep, which is the bug' },
  ]);
  const { code, err } = run(makeRepo(files));
  assert.equal(code, 1);
  assert.match(err, /had to be deep \(limit 30\)/);
});

test('the same late review is accepted when it is deep', () => {
  const files = baseline();
  files['scripts/adr-review-log.json'] = ledger([
    { date: '2026-01-01', kind: 'sweep', next: '2026-02-01', summary: 'first sweep, nothing changed' },
    { date: '2026-06-01', kind: 'deep', next: '2026-08-30', summary: 'deep pass over every owner and code reference' },
  ]);
  const { code, err } = run(makeRepo(files));
  // The fixture is still overdue (that is a separate rule), so only the depth
  // requirement is asserted: nothing may complain that this had to be deep.
  assert.doesNotMatch(err, /had to be deep/);
  if (code !== 0) assert.match(err, /review overdue/);
});

test('a next date beyond the cadence is rejected', () => {
  const files = baseline();
  files['scripts/adr-review-log.json'] = ledger([{ ...REVIEW, next: '2027-06-01' }]);
  const { code, err } = run(makeRepo(files));
  assert.equal(code, 1);
  assert.match(err, /beyond the 90-day cadence/);
});

test('a review with no summary is not a review', () => {
  const files = baseline();
  files['scripts/adr-review-log.json'] = ledger([{ ...REVIEW, summary: '  ' }]);
  const { code, err } = run(makeRepo(files));
  assert.equal(code, 1);
  assert.match(err, /summary is empty/);
});

test('a future-dated review is rejected', () => {
  const files = baseline();
  files['scripts/adr-review-log.json'] = ledger([{ ...REVIEW, date: '2027-01-05', next: '2027-04-05' }]);
  const { code, err } = run(makeRepo(files));
  assert.equal(code, 1);
  assert.match(err, /the review is dated in the future/);
});

test('a missing ledger fails rather than silently dropping the cadence', () => {
  const files = baseline();
  delete files['scripts/adr-review-log.json'];
  const { code, err } = run(makeRepo(files));
  assert.equal(code, 1);
  assert.match(err, /a cadence with no ledger is a cadence nobody keeps/);
});

test('a review due within two weeks warns but still passes', () => {
  const files = baseline();
  files['scripts/adr-review-log.json'] = ledger([{ ...REVIEW, next: '2026-12-05' }]);
  const { code, out } = run(makeRepo(files));
  assert.equal(code, 0, out);
  assert.match(out, /note: review due in 8 day\(s\)/);
});

test('a superseded ADR may not be cited as a current constraint', () => {
  const files = baseline();
  files['docs/decisions/ADR-0001-alpha.md'] = adr('0001', 'alpha', {
    status: 'superseded',
    date: '2026-01-01',
    scope: '长期',
    'superseded-by': 'ADR-0002',
  });
  files['docs/decisions/ADR-0002-beta.md'] = adr('0002', 'beta', {
    status: 'accepted',
    date: '2026-02-01',
    scope: '长期',
    supersedes: 'ADR-0001',
  });
  files['docs/decisions/README.md'] = index(row('0001', 'alpha', 'superseded'), row('0002', 'beta', 'accepted', '2026-02-01'));
  files['docs/roadmap.md'] = '# Roadmap\n\nStill following ADR-0001 for this.\n';
  const { code, err } = run(makeRepo(files));
  assert.equal(code, 1);
  assert.match(err, /cites ADR-0001 as a current constraint, but that ADR is no longer in force/);
});

test('pointing --root at something that is not the repo fails loudly', () => {
  const empty = fs.mkdtempSync(path.join(os.tmpdir(), 'adr-empty-'));
  const { code, err } = run(empty);
  assert.equal(code, 1);
  assert.match(err, /docs\/decisions not found/);
});
