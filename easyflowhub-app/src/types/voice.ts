export type VoiceStatus = 'idle' | 'recording' | 'paused' | 'stopping' | 'saved' | 'failed';

export interface VoiceSession {
  id: string;
  title: string;
  status: VoiceStatus;
  startedAt: string | null;
  stoppedAt: string | null;
  durationMs: number;
  deviceName: string | null;
  sampleRate: number;
  channels: number;
  audioPath: string | null;
  inboxItemId: string | null;
  error: string | null;
}
