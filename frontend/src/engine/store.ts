import type { Client } from './client';
import { Emitter } from './events';
import type { Comp, GroupNode, LayerRow, UiState } from './types';

export interface StoreEvents {
  state: UiState;
  items: UiState;
  comp: UiState;
  layers: UiState;
  selection: UiState;
  time: UiState;
  trees: UiState;
  history: UiState;
  viewer: UiState;
  project: UiState;
}

const json = (v: unknown): string => JSON.stringify(v ?? null);

export class Store {
  readonly events = new Emitter<StoreEvents>();
  state: UiState | null = null;
  private pulling = false;
  private dirty = false;
  private readonly wantedTrees = new Map<string, Set<number>>();
  private sig: Record<string, string> = {};
  playing = false;

  constructor(private readonly client: Client) {
    client.events.on('open', () => this.invalidate());
    client.events.on('changed', () => this.invalidate());
  }

  watch(fn: (st: UiState) => void): void {
    this.events.on('state', fn);
    if (this.state) queueMicrotask(() => this.state && fn(this.state));
  }

  get comp(): Comp | null {
    return this.state?.comp ?? null;
  }

  get layers(): LayerRow[] {
    return this.state?.layers ?? [];
  }

  get time(): number {
    return this.state?.time ?? 0;
  }

  layer(id: number): LayerRow | undefined {
    return this.state?.layers.find((l) => l.id === id);
  }

  tree(id: number): GroupNode | undefined {
    return this.state?.trees[String(id)];
  }

  wantTrees(owner: string, ids: Iterable<number>): void {
    const next = new Set(ids);
    const prev = this.wantedTrees.get(owner);
    if (prev && prev.size === next.size && [...next].every((x) => prev.has(x))) return;
    this.wantedTrees.set(owner, next);
    const st = this.state;
    if (st && [...next].some((id) => !st.trees[String(id)])) this.invalidate();
  }

  private trees(): number[] {
    const all = new Set<number>();
    for (const s of this.wantedTrees.values()) for (const id of s) all.add(id);
    return [...all];
  }

  invalidate(): void {
    if (this.pulling) {
      this.dirty = true;
      return;
    }
    void this.pull();
  }

  async pull(): Promise<void> {
    this.pulling = true;
    try {
      do {
        this.dirty = false;
        const st = await this.client.call<UiState>('ui.state', { trees: this.trees() });
        this.apply(st);
      } while (this.dirty);
    } catch (e) {
      console.warn('[formvfx] state pull failed', e);
    } finally {
      this.pulling = false;
    }
  }

  private apply(st: UiState): void {
    this.state = st;
    const parts: Record<keyof StoreEvents, string> = {
      state: '',
      items: json(st.items) + json(st.projectSelection),
      comp: json(st.comp),
      layers: json(st.layers),
      selection: json(st.selection),
      time: json(st.time),
      trees: json(st.trees),
      history: json([st.undo, st.redo, st.dirty, st.path]),
      viewer: json([st.viewer, st.snapping, st.roi, st.textEdit]),
      project: json([st.path, st.dirty, st.bitDepth, st.settings, st.activeComp, st.openComps]),
    };
    const changed: Array<keyof StoreEvents> = [];
    for (const k of Object.keys(parts) as Array<keyof StoreEvents>) {
      if (k === 'state') continue;
      if (this.sig[k] !== parts[k]) {
        this.sig[k] = parts[k];
        changed.push(k);
      }
    }
    for (const k of changed) this.events.emit(k, st);
    this.events.emit('state', st);
  }
}
