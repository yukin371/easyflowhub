import type { ScriptSummary } from '../../types/scriptmgr';
import type { Action } from './types';

/** Convert a ScriptMgr entry into Launcher metadata without binding execution. */
export function scriptSummaryToAction(script: ScriptSummary): Action {
  return {
    id: `script:${script.id}`,
    name: script.name,
    kind: 'script',
    source: 'script',
    description: script.description,
    keywords: script.tags,
    tags: script.tags,
    category: script.category ?? 'Scripts',
    metadata: {
      scriptId: script.id,
      path: script.path,
      scriptType: script.script_type,
    },
  };
}
