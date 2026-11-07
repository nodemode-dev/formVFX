import { bus } from '../../../../demo/src/state';

export type Tool = 'select' | 'pen' | 'track';

export const T = { tool: 'select' as Tool, drawing: null as string | null };

export function setTool(t: Tool): void {
  T.tool = t;
  T.drawing = null;
  bus.emit('tool');
}
