import { el } from '../../../frontend/src/ui/dom';

export interface Scrub {
  el: HTMLElement;
  update: () => void;
}

export function decimalsFor(step: number): number {
  if (step >= 1) return 0;
  return Math.min(3, Math.max(1, Math.ceil(-Math.log10(step))));
}

export function scrub(o: {
  get: () => number;
  set: (v: number) => void;
  commit: () => void;
  min: number;
  max: number;
  step: number;
  unit: string;
}): Scrub {
  const root = el<HTMLElement>('<span class="fx-value"><span class="fx-scrub" tabindex="0"></span><input class="fx-scrub-input" type="text" hidden></span>');
  const span = root.firstElementChild as HTMLElement;
  const input = root.lastElementChild as HTMLInputElement;
  const dec = decimalsFor(o.step);
  const fmt = (v: number): string => v.toFixed(dec) + o.unit;
  const update = (): void => {
    const s = fmt(o.get());
    if (span.textContent !== s) span.textContent = s;
  };
  let down: { x: number; v: number; moved: boolean } | null = null;
  span.addEventListener('pointerdown', (e) => {
    if (e.button !== 0) return;
    e.preventDefault();
    span.setPointerCapture(e.pointerId);
    down = { x: e.clientX, v: o.get(), moved: false };
    span.classList.add('is-scrubbing');
  });
  span.addEventListener('pointermove', (e) => {
    if (!down) return;
    const dx = e.clientX - down.x;
    if (Math.abs(dx) > 2) down.moved = true;
    if (!down.moved) return;
    const mult = e.shiftKey ? 10 : e.ctrlKey || e.metaKey ? 0.1 : 1;
    const raw = down.v + dx * o.step * mult;
    const v = Math.round(raw / (o.step * (e.ctrlKey ? 0.1 : 1))) * o.step * (e.ctrlKey ? 0.1 : 1);
    o.set(Math.max(o.min, Math.min(o.max, v)));
    update();
  });
  const end = (e: PointerEvent): void => {
    if (!down) return;
    const d = down;
    down = null;
    span.classList.remove('is-scrubbing');
    if (span.hasPointerCapture(e.pointerId)) span.releasePointerCapture(e.pointerId);
    if (d.moved) o.commit();
    else edit();
  };
  span.addEventListener('pointerup', end);
  span.addEventListener('pointercancel', end);
  const edit = (): void => {
    input.value = o.get().toFixed(dec);
    input.hidden = false;
    span.hidden = true;
    input.focus();
    input.select();
  };
  const finish = (apply: boolean): void => {
    if (input.hidden) return;
    const n = parseFloat(input.value);
    input.hidden = true;
    span.hidden = false;
    if (apply && Number.isFinite(n)) { o.set(Math.max(o.min, Math.min(o.max, n))); o.commit(); }
    update();
  };
  input.addEventListener('keydown', (e) => {
    e.stopPropagation();
    if (e.key === 'Enter') finish(true);
    else if (e.key === 'Escape') finish(false);
  });
  input.addEventListener('blur', () => finish(true));
  span.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') edit();
    else if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
      const d = e.key === 'ArrowRight' ? 1 : -1;
      o.set(Math.max(o.min, Math.min(o.max, o.get() + d * o.step * (e.shiftKey ? 10 : 1))));
      update();
      o.commit();
    }
  });
  update();
  return { el: root, update };
}

export function swatch(get: () => string, set: (v: string) => void, commit: () => void): { el: HTMLElement; update: () => void } {
  const root = el<HTMLElement>('<span class="fx-swatch"><input type="color"></span>');
  const input = root.firstElementChild as HTMLInputElement;
  const update = (): void => {
    const v = get();
    if (input.value !== v) input.value = v;
    root.style.setProperty('--sw', v);
  };
  input.addEventListener('input', () => { set(input.value); root.style.setProperty('--sw', input.value); });
  input.addEventListener('change', commit);
  update();
  return { el: root, update };
}

export function selectBox(options: Array<[string, string]>, get: () => string, set: (v: string) => void, cls = 'fx-select fx-mini'): { el: HTMLSelectElement; update: () => void } {
  const s = el<HTMLSelectElement>(`<select class="${cls}"></select>`);
  for (const [v, label] of options) {
    const o = document.createElement('option');
    o.value = v; o.textContent = label;
    s.appendChild(o);
  }
  s.addEventListener('change', () => set(s.value));
  const update = (): void => { const v = get(); if (s.value !== v && document.activeElement !== s) s.value = v; };
  update();
  return { el: s, update };
}
