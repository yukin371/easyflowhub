import { invoke } from '@tauri-apps/api/core';
import type { VoiceSession } from '../../types/voice';

export const voiceUnavailableMessage = '当前平台未启用 Windows 麦克风录音后端；没有生成或上传音频。';

export const getVoiceState = () => invoke<VoiceSession>('voice_get_state');
export const startVoice = (title?: string) => invoke<VoiceSession>('voice_start', { title: title?.trim() || null });
export const pauseVoice = () => invoke<VoiceSession>('voice_pause');
export const resumeVoice = () => invoke<VoiceSession>('voice_resume');
export const stopVoice = () => invoke<VoiceSession>('voice_stop');
export const openVoice = (id: string) => invoke<void>('voice_open', { id });
export const openVoicePrivacySettings = () => invoke<void>('voice_open_privacy_settings');
