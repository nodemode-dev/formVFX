import { el, setClass, setText } from './dom';

export interface ScrubOptions {
  value: number;
  step?: number;
  min?: number;
  max?: number;
  decimals?: number;
  suffix?: string;
  format?: (v: number) => string;
  parse?: (s: string) => number | null;
  onBegin?: () => void;
  onChange: (v: number, live: boolean) => void;
  onEnd?: () => void;
  id?: string;
}

const clamp = (v: number, lo?: number, hi?: number): number => Math.min(hi ?? Infinity, Math.max(lo ?? -Infinity, v));

export class Scrub {
  readonly el: HTMLElement;
  private input: HTMLInputElement | null = null;
  value: number;

  constructor(private readonly o: ScrubOptions) {
    this.value = o.value;
    this.el = el('<span class="fx-scrub" tabindex="0" role="spinbutton"></span>');
    if (o.id) this.el.dataset.automationId = o.id;
    this.render();
    this.el.addEventListener('pointerdown', (e) => this.down(e));
    this.el.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        this.edit();
      } else if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
        e.preventDefault();
        const st = (o.step ?? 1) * (e.shiftKey ? 10 : 1) * (e.key === 'ArrowUp' ? 1 : -1);
        this.commit(this.value + st, false);
      }
    });
  }

  private fmt(v: number): string {
    if (this.o.format) return this.o.format(v);
    const d = this.o.decimals ?? 1;
    return `${Number.isFinite(v) ? v.toFixed(d) : '0'}${this.o.suffix ?? ''}`;
  }

  set(v: number): void {
    if (this.input) return;
    this.value = v;
    this.render();
  }

  private render(): void {
    setText(this.el, this.fmt(this.value));
  }

  private commit(v: number, live: boolean): void {
    const c = clamp(v, this.o.min, this.o.max);
    this.value = c;
    this.render();
    this.o.onChange(c, live);
  }

  private down(e: PointerEvent): void {
    if (e.button !== 0 || this.input) return;
    e.preventDefault();
    const start = { x: e.clientX, v: this.value };
    let moved = false;
    this.el.setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent): void => {
      const dx = ev.clientX - start.x;
      if (!moved && Math.abs(dx) < 3) return;
      if (!moved) {
        moved = true;
        setClass(this.el, 'is-scrubbing', true);
        this.o.onBegin?.();
      }
      const mul = ev.shiftKey ? 10 : ev.ctrlKey || ev.metaKey ? 0.1 : 1;
      this.commit(start.v + dx * (this.o.step ?? 1) * mul, true);
    };
    const up = (): void => {
      this.el.removeEventListener('pointermove', move);
      this.el.removeEventListener('pointerup', up);
      this.el.removeEventListener('pointercancel', up);
      setClass(this.el, 'is-scrubbing', false);
      if (moved) this.o.onEnd?.();
      else this.edit();
    };
    this.el.addEventListener('pointermove', move);
    this.el.addEventListener('pointerup', up);
    this.el.addEventListener('pointercancel', up);
  }

  edit(): void {
    if (this.input) return;
    const w = Math.max(48, this.el.offsetWidth + 12);
    const input = el<HTMLInputElement>('<input class="fx-scrub-input" type="text" spellcheck="false">');
    input.style.width = `${w}px`;
    input.value = this.o.format ? this.fmt(this.value) : String(Number(this.value.toFixed(this.o.decimals ?? 3)));
    this.input = input;
    this.el.replaceChildren(input);
    input.focus();
    input.select();
    const done = (ok: boolean): void => {
      if (!this.input) return;
      const raw = input.value.trim();
      this.input = null;
      this.render();
      if (!ok) return;
      let v: number | null = this.o.parse ? this.o.parse(raw) : null;
      if (v === null) {
        try {
          v = /^[-+*/0-9.()\s]+$/.test(raw) ? Number(Function(`"use strict";return (${raw})`)()) : Number(raw);
        } catch {
          v = Number(raw);
        }
      }
      if (v !== null && Number.isFinite(v)) {
        this.o.onBegin?.();
        this.commit(v, false);
        this.o.onEnd?.();
      }
    };
    input.addEventListener('keydown', (e) => {
      e.stopPropagation();
      if (e.key === 'Enter') done(true);
      if (e.key === 'Escape') done(false);
      if (e.key === 'Tab') done(true);
    });
    input.addEventListener('blur', () => done(true));
  }
}

export function colorToHex(v: unknown): string {
  if (typeof v === 'string' && /^#[0-9a-f]{6}/i.test(v)) return v.slice(0, 7);
  if (Array.isArray(v) && v.length >= 3) {
    return '#' + v.slice(0, 3).map((c) => Math.round(Math.max(0, Math.min(1, Number(c))) * 255).toString(16).padStart(2, '0')).join('');
  }
  return '#000000';
}

export function hexToRgb(h: string): [number, number, number] {
  const n = parseInt(h.replace('#', '').slice(0, 6), 16);
  return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
}

export class Swatch {
  readonly el: HTMLLabelElement;
  private readonly input: HTMLInputElement;

  constructor(value: unknown, onChange: (rgb: [number, number, number], live: boolean) => void, id?: string) {
    this.el = el<HTMLLabelElement>('<label class="fx-swatch"><input type="color"></label>');
    this.input = this.el.querySelector('input') as HTMLInputElement;
    if (id) this.el.dataset.automationId = id;
    this.set(value);
    this.input.addEventListener('input', () => onChange(hexToRgb(this.input.value), true));
    this.input.addEventListener('change', () => onChange(hexToRgb(this.input.value), false));
  }

  set(v: unknown): void {
    const hex = colorToHex(v);
    if (this.input.value !== hex && document.activeElement !== this.input) this.input.value = hex;
    this.el.style.setProperty('--sw', hex);
  }
}

export function checkbox(on: boolean, onChange: (v: boolean) => void, id?: string): HTMLInputElement {
  const c = el<HTMLInputElement>('<input type="checkbox" class="fx-check">');
  c.checked = on;
  if (id) c.dataset.automationId = id;
  c.addEventListener('change', () => onChange(c.checked));
  return c;
}

export function select(options: string[], value: number | string, onChange: (index: number, label: string) => void, id?: string): HTMLSelectElement {
  const s = el<HTMLSelectElement>('<select class="fx-select"></select>');
  options.forEach((o, i) => {
    const opt = el<HTMLOptionElement>('<option></option>');
    opt.value = String(i);
    opt.textContent = o;
    s.appendChild(opt);
  });
  s.value = typeof value === 'number' ? String(value) : String(Math.max(0, options.indexOf(value)));
  if (id) s.dataset.automationId = id;
  s.addEventListener('change', () => onChange(Number(s.value), options[Number(s.value)]));
  return s;
}

export function angleText(v: number): string {
  const turns = Math.trunc(v / 360);
  const rest = v - turns * 360;
  return `${turns}x${rest >= 0 ? '+' : ''}${rest.toFixed(1)}°`;
}

export function parseAngle(s: string): number | null {
  const m = /^\s*([-+]?\d+)\s*x\s*([-+]?\d*\.?\d+)\s*°?\s*$/i.exec(s);
  if (m) return Number(m[1]) * 360 + Number(m[2]);
  const n = parseFloat(s);
  return Number.isFinite(n) ? n : null;
}
