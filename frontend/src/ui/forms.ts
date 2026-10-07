import type { CommandSpec, UiState } from '../engine/types';
import { Dialog } from './dialogs';
import { el, escapeHtml } from './dom';

export interface ParamField {
  name: string;
  optional: boolean;
  options: string[] | null;
  hint: string;
}

const NUMERIC = /^(width|height|duration|frameRate|time|frames?|index|size|amount|radius|angle|rotation|opacity|scale|x|y|z|start|end|delta|in|out|offset|frequency|magnitude|seed|tolerance|speed|influence|factor|page|delay|threshold|stretch|position|overlap|feather|expansion|count|zoom|intensity|depth|samples|quality|leading|tracking|strokeWidth|fontSize|pixelAspect|steps)$/i;
const BOOL = /\b(bool|true|false)\b/i;
const COLOR = /^(color|background|fill|stroke|matteColor|tint)$/i;

export function parseParamDoc(doc: string): ParamField[] {
  const body = doc.trim().replace(/^\{/, '').replace(/\}.*$/s, '');
  const parts: string[] = [];
  let depth = 0;
  let cur = '';
  for (const ch of body) {
    if (ch === '(' || ch === '[' || ch === '{') depth++;
    if (ch === ')' || ch === ']' || ch === '}') depth--;
    if (ch === ',' && depth === 0) {
      parts.push(cur);
      cur = '';
    } else cur += ch;
  }
  if (cur.trim()) parts.push(cur);
  const out: ParamField[] = [];
  for (const raw of parts) {
    const p = raw.trim().replace(/…$/, '').trim();
    if (!p || p === '...' || p === '…') continue;
    const m = /^([A-Za-z_][A-Za-z0-9_]*)(\?)?(?:\s*\|\s*([A-Za-z_][A-Za-z0-9_]*)\??)?\s*(?::\s*(.*))?$/.exec(p.replace(/\s*\(.*\)\s*$/, ''));
    if (!m) continue;
    const rest = m[4] || '';
    const hint = (/\((.*)\)/.exec(p) || [])[1] || rest;
    let options: string[] | null = null;
    const choice = /^([A-Za-z0-9_-]+(?:\s*\|\s*[A-Za-z0-9_-]+)+)/.exec(rest);
    if (choice) options = choice[1].split('|').map((s) => s.trim());
    out.push({ name: m[1], optional: !!m[2], options, hint: hint.trim() });
  }
  return out;
}

function guessKind(f: ParamField): 'number' | 'bool' | 'color' | 'select' | 'json' | 'text' {
  if (f.options) return 'select';
  if (COLOR.test(f.name)) return 'color';
  if (BOOL.test(f.hint)) return 'bool';
  if (NUMERIC.test(f.name)) return 'number';
  if (/\[|\{/.test(f.hint) || /s$/.test(f.name)) return 'json';
  return 'text';
}

const toHex = (v: unknown): string => {
  if (typeof v === 'string' && /^#[0-9a-f]{6}$/i.test(v)) return v;
  if (Array.isArray(v) && v.length >= 3) return '#' + v.slice(0, 3).map((c) => Math.round(Math.max(0, Math.min(1, Number(c))) * 255).toString(16).padStart(2, '0')).join('');
  return '#808080';
};

export function prefill(command: string, st: UiState | null): Record<string, unknown> {
  const comp = st?.comp;
  switch (command) {
    case 'comp.new':
      return { name: `Comp ${(st?.items.filter((i) => i.type === 'Composition').length ?? 0) + 1}`, width: 1920, height: 1080, frameRate: 30, duration: 10, background: '#000000' };
    case 'comp.settings':
      return comp ? { name: comp.name, width: comp.width, height: comp.height, frameRate: comp.frameRate, duration: comp.duration, background: toHex(comp.background) } : {};
    case 'layer.newSolid':
      return { name: 'Solid', color: '#7f7f7f', width: comp?.width ?? 1920, height: comp?.height ?? 1080 };
    case 'time.set':
      return { time: st?.time ?? 0 };
    default:
      return {};
  }
}

export interface FormResult {
  params: Record<string, unknown>;
}

export function commandForm(spec: CommandSpec, initial: Record<string, unknown>): Promise<FormResult | null> {
  const fields = parseParamDoc(spec.params || '{}').filter((f) => f.name !== 'merge');
  const dialog = new Dialog(spec.label.replace(/\.\.\.$/, '').replace(/…$/, ''), 460);
  dialog.scrim.dataset.automationId = `form.${spec.id}`;
  const inputs: Array<{ f: ParamField; kind: string; node: HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement }> = [];
  if (!fields.length) {
    dialog.body.appendChild(el(`<p class="cv-note">${escapeHtml(spec.label)} takes no settings.</p>`));
  }
  for (const f of fields) {
    const kind = guessKind(f);
    let node: HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement;
    const v = initial[f.name];
    if (kind === 'select') {
      const s = el<HTMLSelectElement>('<select class="cv-input fx-select"></select>');
      if (f.optional) s.appendChild(el('<option value="">(default)</option>'));
      for (const o of f.options || []) {
        const opt = el<HTMLOptionElement>('<option></option>');
        opt.value = o;
        opt.textContent = o;
        s.appendChild(opt);
      }
      if (typeof v === 'string') s.value = v;
      node = s;
    } else if (kind === 'bool') {
      const c = el<HTMLInputElement>('<input type="checkbox" class="fx-check">');
      c.checked = v === true;
      node = c;
    } else if (kind === 'color') {
      const c = el<HTMLInputElement>('<input type="color" class="fx-color">');
      c.value = toHex(v);
      node = c;
    } else if (kind === 'json') {
      const t = el<HTMLTextAreaElement>('<textarea class="cv-input fx-json" rows="2" spellcheck="false"></textarea>');
      t.value = v === undefined ? '' : JSON.stringify(v);
      node = t;
    } else {
      const i = el<HTMLInputElement>(`<input class="cv-input" type="${kind === 'number' ? 'number' : 'text'}" step="any" spellcheck="false">`);
      i.value = v === undefined || v === null ? '' : String(v);
      node = i;
    }
    node.dataset.automationId = `form.${spec.id}.${f.name}`;
    const row = dialog.field(labelOf(f.name) + (f.optional ? '' : ' *'), node);
    if (f.hint && kind !== 'select') row.title = f.hint;
    inputs.push({ f, kind, node });
  }
  return new Promise((resolve) => {
    let done = false;
    const finish = (r: FormResult | null): void => {
      if (done) return;
      done = true;
      dialog.close();
      dialog.scrim.remove();
      resolve(r);
    };
    dialog.onClose = () => finish(null);
    dialog.button('Cancel', 'ghost', () => finish(null));
    const ok = dialog.button('OK', 'primary', () => {
      const params: Record<string, unknown> = { ...initial };
      for (const { f, kind, node } of inputs) {
        if (kind === 'bool') {
          params[f.name] = (node as HTMLInputElement).checked;
          continue;
        }
        const raw = node.value.trim();
        if (!raw) {
          delete params[f.name];
          continue;
        }
        if (kind === 'number') params[f.name] = Number(raw);
        else if (kind === 'json') {
          try {
            params[f.name] = JSON.parse(raw);
          } catch {
            params[f.name] = raw;
          }
        } else params[f.name] = raw;
      }
      finish({ params });
    });
    dialog.scrim.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' && !(e.target instanceof HTMLTextAreaElement)) {
        e.preventDefault();
        ok.click();
      }
    });
    dialog.open();
  });
}

function labelOf(name: string): string {
  const s = name.replace(/([A-Z])/g, ' $1');
  return s.charAt(0).toUpperCase() + s.slice(1);
}
