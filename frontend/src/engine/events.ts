export type Listener<T> = (value: T) => void;

export class Emitter<M extends object> {
  private readonly map = new Map<keyof M, Set<Listener<never>>>();

  on<K extends keyof M>(key: K, fn: Listener<M[K]>): () => void {
    let set = this.map.get(key);
    if (!set) {
      set = new Set();
      this.map.set(key, set);
    }
    set.add(fn as Listener<never>);
    return () => set?.delete(fn as Listener<never>);
  }

  emit<K extends keyof M>(key: K, value: M[K]): void {
    const set = this.map.get(key);
    if (!set) return;
    for (const fn of Array.from(set)) {
      try {
        (fn as Listener<M[K]>)(value);
      } catch (e) {
        console.error(`[formvfx] listener for ${String(key)} failed`, e);
      }
    }
  }
}
