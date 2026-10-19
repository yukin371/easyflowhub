import { VoicePanel } from '../../components/manager/VoicePanel';
import { defineBuiltinModule } from './defineBuiltinModule';

export const voiceModule = defineBuiltinModule({ id: 'voice', name: '录音', icon: '音', caption: 'Voice', defaultEnabled: true }, VoicePanel);
