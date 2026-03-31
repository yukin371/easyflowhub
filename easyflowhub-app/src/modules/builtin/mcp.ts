/**
 * MCP 模块定义
 */
import type { FeatureModule } from '../types';
import { McpPanel } from '../../components/manager/mcp/McpPanel';

export const mcpModule: FeatureModule = {
  id: 'mcp',
  name: 'MCP',
  icon: '链',
  caption: 'MCP Server',
  defaultEnabled: false,
  component: McpPanel,
};
