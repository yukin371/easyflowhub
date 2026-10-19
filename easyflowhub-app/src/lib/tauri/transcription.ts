import { invoke } from '@tauri-apps/api/core';

export interface TranscriptionResult {
  text: string;
  model: string;
  attachmentId: string;
  audioSizeBytes: number;
  durationMs: number | null;
}

export interface InboxTranscript {
  attachmentId: string;
  text: string;
  model: string | null;
  createdAt: string;
}

export const transcribeAudio = (itemId: string) => invoke<TranscriptionResult>('transcribe_audio', { itemId });
export const getTranscript = (itemId: string) => invoke<InboxTranscript | null>('get_transcript', { itemId });
