/**
 * MCP 模块定义
 */
import { McpPanel } from '../../components/manager/mcp/McpPanel';
import { defineBuiltinModule } from './defineBuiltinModule';

export const mcpModule = defineBuiltinModule(
  {
    id: 'mcp',
    name: 'MCP',
    icon: '链',
    caption: 'MCP Server',
    defaultEnabled: false,
  },
  McpPanel
);
