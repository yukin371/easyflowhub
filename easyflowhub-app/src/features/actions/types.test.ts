import { describe, expect, it, expectTypeOf } from 'vitest';
import type {
  Action,
  ActionContextSnapshot,
  ActionExecutionContext,
  ActionExecutionState,
  ActionHandlerResult,
  ActionInputSpec,
} from './types';
import type { CaptureAttachment } from '../../types/capture';

const context: ActionExecutionContext = {
  capturedAt: '2026-10-18T08:00:00.000Z',
  clipboard: { text: 'selected material' },
  selectedText: 'selected material',
  currentFile: { path: 'C:/work/notes.md', mimeType: 'text/markdown' },
  screenshot: {
    source: 'external',
    capturedAt: '2026-10-18T08:00:00.000Z',
    image: { mimeType: 'image/png', width: 1920, height: 1080 },
  },
  window: { processName: 'editor.exe', processId: 42 },
  capture: {
    id: 'capture-1',
    kind: 'text',
    source: 'launcher',
    capturedAt: '2026-10-18T08:00:00.000Z',
    attachments: [
      { id: 'image-1', kind: 'image', source: 'clipboard', createdAt: '2026-10-18T08:00:00.000Z', inboxItemId: null, mimeType: 'image/png', sizeBytes: 1200, location: { kind: 'data-url', value: 'data:image/png;base64,AA==' } },
      { id: 'file-1', kind: 'file', source: 'manager', createdAt: '2026-10-18T08:00:00.000Z', inboxItemId: 'inbox-1', mimeType: 'text/plain', location: { kind: 'local-path', value: 'C:/work/notes.txt' } },
      { id: 'audio-1', kind: 'audio', source: 'voice', createdAt: '2026-10-18T08:00:00.000Z', inboxItemId: null, mimeType: 'audio/wav', location: { kind: 'local-path', value: 'C:/voice/recording.wav.part' } },
      { id: 'link-1', kind: 'link', source: 'clipboard', createdAt: '2026-10-18T08:00:00.000Z', inboxItemId: null, url: 'https://example.test/' },
      { id: 'text-1', kind: 'text', source: 'launcher', createdAt: '2026-10-18T08:00:00.000Z', inboxItemId: null, content: 'Captured text' },
      { id: 'transcript-1', kind: 'transcript', source: 'provider', createdAt: '2026-10-18T08:00:00.000Z', inboxItemId: 'inbox-1', content: 'recognized words', audioAttachmentId: 'audio-1' },
    ],
  },
};

const action: Action = {
  id: 'builtin.format-selection',
  name: 'Format selection',
  kind: 'text',
  inputs: [{
    id: 'selection',
    label: 'Selected text',
    kind: 'text',
    source: 'selection',
    permissions: ['selection.read'],
    schema: { minLength: 1, maxLength: 20_000 },
  }],
  outputs: ['clipboard', 'inbox'],
};

const result: ActionHandlerResult = {
  ok: false,
  status: 'error',
  error: 'The local provider is unavailable.',
  retryable: true,
};

// @ts-expect-error permissions must use a declared capability.
const invalidPermission: ActionInputSpec = { id: 'x', label: 'X', kind: 'text', permissions: ['camera.capture'] };
// @ts-expect-error executor outputs must use a declared result kind.
const invalidOutput: ActionHandlerResult = { ok: true, status: 'success', output: { kind: 'gps' } };
// @ts-expect-error every attachment must retain its future Inbox relationship explicitly.
const missingInboxLink: CaptureAttachment = { id: 'orphan', kind: 'link', source: 'clipboard', createdAt: '2026-10-18T08:00:00.000Z', url: 'https://example.test/' };

void invalidPermission;
void invalidOutput;
void missingInboxLink;

describe('action contract types', () => {
  it('models the supported desktop context snapshots', () => {
    const snapshot: ActionContextSnapshot = context;
    expect(snapshot).toMatchObject({
      currentFile: { path: 'C:/work/notes.md' },
      screenshot: { source: 'external' },
      window: { processName: 'editor.exe' },
    });
    expect(snapshot.capture?.attachments.map(({ kind }) => kind)).toEqual([
      'image', 'file', 'audio', 'link', 'text', 'transcript',
    ]);
    expectTypeOf(snapshot.clipboard?.text).toEqualTypeOf<string | undefined>();
    expectTypeOf(snapshot.screenshot?.image?.width).toEqualTypeOf<number | undefined>();
  });

  it('declares input permission, parameter and output contracts', () => {
    expect(action.inputs?.[0]).toMatchObject({
      source: 'selection',
      permissions: ['selection.read'],
      schema: { minLength: 1, maxLength: 20_000 },
    });
    expect(action.outputs).toEqual(['clipboard', 'inbox']);
    expect(result.retryable).toBe(true);
    expectTypeOf<ActionExecutionState>().toEqualTypeOf<
      'running' | 'success' | 'error' | 'cancelled' | 'unavailable'
    >();
  });
});
