import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { VOICE_NAVIGATE_INBOX_EVENT, VoicePanel } from './VoicePanel';
import type { VoiceSession } from '../../types/voice';

const getVoiceState = vi.hoisted(() => vi.fn());
const startVoice = vi.hoisted(() => vi.fn());
const pauseVoice = vi.hoisted(() => vi.fn());
const resumeVoice = vi.hoisted(() => vi.fn());
const stopVoice = vi.hoisted(() => vi.fn());
const openVoice = vi.hoisted(() => vi.fn());
const openVoicePrivacySettings = vi.hoisted(() => vi.fn());
const convertInboxItemToNote = vi.hoisted(() => vi.fn());
const getTranscript = vi.hoisted(() => vi.fn());
const transcribeAudio = vi.hoisted(() => vi.fn());
const getSettings = vi.hoisted(() => vi.fn());
const copyTextToClipboard = vi.hoisted(() => vi.fn());
vi.mock('../../lib/tauri/voice', () => ({ getVoiceState, startVoice, pauseVoice, resumeVoice, stopVoice, openVoice, openVoicePrivacySettings }));
vi.mock('../../lib/tauri/inbox', () => ({ convertInboxItemToNote }));
vi.mock('../../lib/tauri/transcription', () => ({ getTranscript, transcribeAudio }));
vi.mock('../../lib/tauri/settings', () => ({ getSettings }));
vi.mock('../../lib/tauri/clipboard', () => ({ copyTextToClipboard }));

const makeSession = (overrides: Partial<VoiceSession> = {}): VoiceSession => ({
  id: 'voice-1', title: '会议', status: 'idle', startedAt: null, stoppedAt: null,
  durationMs: 0, deviceName: null, sampleRate: 48_000, channels: 1,
  audioPath: null, inboxItemId: null, error: null, ...overrides,
});

describe('VoicePanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getVoiceState.mockResolvedValue(makeSession());
    startVoice.mockResolvedValue(makeSession({ status: 'recording', deviceName: '麦克风', startedAt: '2026-01-01T00:00:00Z' }));
    pauseVoice.mockResolvedValue(makeSession({ status: 'paused', deviceName: '麦克风', durationMs: 1_200 }));
    resumeVoice.mockResolvedValue(makeSession({ status: 'recording', deviceName: '麦克风', durationMs: 1_200 }));
    stopVoice.mockResolvedValue(makeSession({ status: 'saved', deviceName: '麦克风', durationMs: 2_300, audioPath: 'C:/recording.wav', inboxItemId: 'inbox-1' }));
    openVoice.mockResolvedValue(undefined);
    openVoicePrivacySettings.mockResolvedValue(undefined);
    convertInboxItemToNote.mockResolvedValue({ ok: true, itemId: 'inbox-1', noteId: 'note-1' });
    getTranscript.mockResolvedValue(null);
    transcribeAudio.mockResolvedValue({ text: '转写文本', model: 'whisper-1', attachmentId: 't1', audioSizeBytes: 96_000, durationMs: 1_000 });
    getSettings.mockResolvedValue({ quick_note: {}, trash: {}, editor: {}, todo: {}, launcher_trigger: {}, translation: {}, transcription: { model: 'whisper-1', language: '' } });
    copyTextToClipboard.mockResolvedValue(undefined);
  });

  it('shows an explicit error when recording is unavailable', async () => {
    startVoice.mockRejectedValueOnce(new Error('Windows 麦克风录音后端尚未启用；没有生成或上传音频。'));
    render(<VoicePanel />);
    await waitFor(() => expect(screen.getByText('状态')).toBeTruthy());
    fireEvent.click(screen.getByRole('button', { name: '开始录音' }));
    expect((await screen.findByRole('alert')).textContent).toContain('尚未启用');
    expect(screen.getByText('待机')).toBeTruthy();
  });

  it('supports pause, resume and stop lifecycle actions', async () => {
    getVoiceState.mockResolvedValueOnce(makeSession({ status: 'recording', deviceName: '麦克风' }));
    render(<VoicePanel />);
    await waitFor(() => expect(screen.getByText('录音中')).toBeTruthy());
    fireEvent.click(screen.getByRole('button', { name: '暂停' }));
    await waitFor(() => expect(screen.getByText('已暂停')).toBeTruthy());
    expect(pauseVoice).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: '继续录音' }));
    await waitFor(() => expect(screen.getByText('录音中')).toBeTruthy());
    expect(resumeVoice).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: '停止并保存' }));
    await waitFor(() => expect(screen.getByText('已保存')).toBeTruthy());
    expect(stopVoice).toHaveBeenCalledTimes(1);
    expect(screen.getByText(/C:\/recording\.wav/)).toBeTruthy();
  });

  it('opens a saved recording and emits an inbox navigation event', async () => {
    getVoiceState.mockResolvedValueOnce(makeSession({ status: 'saved', audioPath: 'C:/recording.wav', inboxItemId: 'inbox-1' }));
    const eventSpy = vi.fn();
    window.addEventListener(VOICE_NAVIGATE_INBOX_EVENT, eventSpy);
    render(<VoicePanel />);
    await waitFor(() => expect(screen.getByText('已保存')).toBeTruthy());
    fireEvent.click(screen.getByRole('button', { name: '打开录音文件' }));
    await waitFor(() => expect(openVoice).toHaveBeenCalledWith('voice-1'));
    fireEvent.click(screen.getByRole('button', { name: '查看收集箱' }));
    expect(eventSpy).toHaveBeenCalledTimes(1);
    window.removeEventListener(VOICE_NAVIGATE_INBOX_EVENT, eventSpy);
  });

  it('requires explicit provider and privacy confirmation before transcribing', async () => {
    getVoiceState.mockResolvedValueOnce(makeSession({ status: 'saved', audioPath: 'C:/recording.wav', inboxItemId: 'inbox-1' }));
    render(<VoicePanel />);
    await waitFor(() => expect(screen.getByText('已保存')).toBeTruthy());

    await waitFor(() => expect(screen.getByRole('button', { name: '转写录音' }).hasAttribute('disabled')).toBe(false));
    fireEvent.click(screen.getByRole('button', { name: '转写录音' }));
    // Nothing is sent before the user confirms.
    expect(transcribeAudio).not.toHaveBeenCalled();
    expect(screen.getByText(/隐私提示/).textContent).toContain('未确认前不会发送');
    expect(screen.getByText(/whisper-1/)).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: '确认并发送' }));
    await waitFor(() => expect(transcribeAudio).toHaveBeenCalledWith('inbox-1'));
    await waitFor(() => expect(screen.getByText('转写文本')).toBeTruthy());
    expect(getTranscript).toHaveBeenCalledWith('inbox-1');
  });

  it('cancelling the confirmation does not transcribe', async () => {
    getVoiceState.mockResolvedValueOnce(makeSession({ status: 'saved', audioPath: 'C:/recording.wav', inboxItemId: 'inbox-1' }));
    render(<VoicePanel />);
    await waitFor(() => expect(screen.getByText('已保存')).toBeTruthy());
    await waitFor(() => expect(screen.getByRole('button', { name: '转写录音' }).hasAttribute('disabled')).toBe(false));
    fireEvent.click(screen.getByRole('button', { name: '转写录音' }));
    fireEvent.click(screen.getByRole('button', { name: '取消' }));
    expect(transcribeAudio).not.toHaveBeenCalled();
    expect(screen.queryByText(/隐私提示/)).toBeNull();
  });

  it('guides the user to settings when no transcription model is configured', async () => {
    getSettings.mockResolvedValue({ quick_note: {}, trash: {}, editor: {}, todo: {}, launcher_trigger: {}, translation: {}, transcription: { model: '', language: '' } });
    getVoiceState.mockResolvedValueOnce(makeSession({ status: 'saved', audioPath: 'C:/recording.wav', inboxItemId: 'inbox-1' }));
    render(<VoicePanel />);
    await waitFor(() => expect(screen.getByText('已保存')).toBeTruthy());
    await waitFor(() => expect(screen.getByRole('button', { name: '转写录音' }).hasAttribute('disabled')).toBe(true));
    expect(screen.getByText(/设置 → 转写/)).toBeTruthy();
  });

  it('surfaces transcription failures without losing the recording', async () => {
    transcribeAudio.mockRejectedValueOnce(new Error('无法连接 Relay，请确认本机 Relay 已启动且地址可访问。'));
    getVoiceState.mockResolvedValueOnce(makeSession({ status: 'saved', audioPath: 'C:/recording.wav', inboxItemId: 'inbox-1' }));
    render(<VoicePanel />);
    await waitFor(() => expect(screen.getByText('已保存')).toBeTruthy());
    await waitFor(() => expect(screen.getByRole('button', { name: '转写录音' }).hasAttribute('disabled')).toBe(false));
    fireEvent.click(screen.getByRole('button', { name: '转写录音' }));
    fireEvent.click(screen.getByRole('button', { name: '确认并发送' }));
    expect((await screen.findByRole('alert')).textContent).toContain('无法连接 Relay');
    expect(screen.getByRole('button', { name: '打开录音文件' })).toBeTruthy();
  });

  it('converts a transcribed recording into a note', async () => {
    getVoiceState.mockResolvedValueOnce(makeSession({ status: 'saved', audioPath: 'C:/recording.wav', inboxItemId: 'inbox-1' }));
    const eventSpy = vi.fn();
    window.addEventListener(VOICE_NAVIGATE_INBOX_EVENT, eventSpy);
    render(<VoicePanel />);
    await waitFor(() => expect(screen.getByText('已保存')).toBeTruthy());
    await waitFor(() => expect(screen.getByRole('button', { name: '转写录音' }).hasAttribute('disabled')).toBe(false));
    fireEvent.click(screen.getByRole('button', { name: '转写录音' }));
    fireEvent.click(screen.getByRole('button', { name: '确认并发送' }));
    await waitFor(() => expect(screen.getByText('转写文本')).toBeTruthy());
    fireEvent.click(screen.getByRole('button', { name: '整理为笔记' }));
    await waitFor(() => expect(convertInboxItemToNote).toHaveBeenCalledWith('inbox-1'));
    await waitFor(() => expect(screen.getByText('已整理为笔记，可在“笔记”页面查看')).toBeTruthy());
    expect(eventSpy).toHaveBeenCalled();
    window.removeEventListener(VOICE_NAVIGATE_INBOX_EVENT, eventSpy);
  });
});
