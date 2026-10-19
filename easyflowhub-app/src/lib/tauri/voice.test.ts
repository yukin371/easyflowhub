import { describe, expect, it, vi } from 'vitest';
import { getVoiceState, openVoice, openVoicePrivacySettings, pauseVoice, resumeVoice, startVoice, stopVoice, voiceUnavailableMessage } from './voice';

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

describe('voice bridge', () => {
  it('uses typed command names and camelCase payloads', async () => {
    invokeMock.mockResolvedValueOnce({ status: 'idle' });
    await getVoiceState();
    expect(invokeMock).toHaveBeenCalledWith('voice_get_state');
    invokeMock.mockResolvedValueOnce({ status: 'failed' });
    await startVoice('会议');
    expect(invokeMock).toHaveBeenCalledWith('voice_start', { title: '会议' });
  });

  it('exposes a truthful unavailable message for unsupported native capture', () => {
    expect(voiceUnavailableMessage).toContain('当前平台未启用');
  });

  it('exposes lifecycle and file commands through the typed bridge', async () => {
    invokeMock.mockClear();
    invokeMock.mockResolvedValue({ status: 'recording' });
    await pauseVoice();
    await resumeVoice();
    await stopVoice();
    await openVoice('voice-1');
    await openVoicePrivacySettings();
    expect(invokeMock).toHaveBeenNthCalledWith(1, 'voice_pause');
    expect(invokeMock).toHaveBeenNthCalledWith(2, 'voice_resume');
    expect(invokeMock).toHaveBeenNthCalledWith(3, 'voice_stop');
    expect(invokeMock).toHaveBeenNthCalledWith(4, 'voice_open', { id: 'voice-1' });
    expect(invokeMock).toHaveBeenNthCalledWith(5, 'voice_open_privacy_settings');
  });
});
