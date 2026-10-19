import { describe, expect, it, vi } from 'vitest';
import { getTranscript, transcribeAudio } from './transcription';

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

describe('transcription bridge', () => {
  it('transcribes an inbox item through the typed command with a camelCase payload', async () => {
    invokeMock.mockResolvedValueOnce({ text: '转写文本', model: 'whisper-1', attachmentId: 'a1', audioSizeBytes: 96000, durationMs: 1000 });
    const result = await transcribeAudio('inbox-1');

    expect(invokeMock).toHaveBeenCalledWith('transcribe_audio', { itemId: 'inbox-1' });
    expect(result.text).toBe('转写文本');
    expect(result.durationMs).toBe(1000);
  });

  it('reads an existing transcript and accepts an empty result', async () => {
    invokeMock.mockResolvedValueOnce({ attachmentId: 'a1', text: '已转写', model: 'whisper-1', createdAt: '2026-10-19T10:00:00Z' });
    const transcript = await getTranscript('inbox-1');

    expect(invokeMock).toHaveBeenCalledWith('get_transcript', { itemId: 'inbox-1' });
    expect(transcript?.text).toBe('已转写');

    invokeMock.mockResolvedValueOnce(null);
    await expect(getTranscript('inbox-2')).resolves.toBeNull();
  });
});
