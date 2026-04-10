import { RelayPanel } from '../../components/manager/relay/RelayPanel';
import { defineBuiltinModule } from './defineBuiltinModule';

export const relayModule = defineBuiltinModule(
  {
    id: 'relay',
    name: '中转',
    icon: '衡',
    caption: 'Relay',
    defaultEnabled: false,
  },
  RelayPanel
);
