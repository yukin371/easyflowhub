import type { FeatureModule } from '../types';
import { RelayPanel } from '../../components/manager/relay/RelayPanel';

export const relayModule: FeatureModule = {
  id: 'relay',
  name: '中转',
  icon: '衡',
  caption: 'Relay',
  defaultEnabled: false,
  component: RelayPanel,
};
