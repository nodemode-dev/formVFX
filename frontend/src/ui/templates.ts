import type { Ctx } from '../app/ctx';
import { Dialog } from './dialogs';
import { el, patchList, q, setText, setTitle } from './dom';
import { contextMenu } from './popup';

interface TemplateRow {
  id: string;
  name: string;
  description?: string;
  category?: string;
  builtin: boolean;
  width?: number;
  height?: number;
  duration?: number;
  controls?: number;
  error?: string;
}

interface Thumb {
  width: number;
  height: number;
  rgb: string;
}

export class TemplateGallery {
  private dialog: Dialog | null = null;
  private grid: HTMLElement | null = null;
  private painted = new Set<string>();
  private pass = 0;

  constructor(private readonly ctx: Ctx) {}

  async open(): Promise<void> {
    if (!this.dialog) this.build();
    this.dialog?.open();
    await this.load();
  }

  private build(): void {
    const d = new Dialog('New Project from Template', 760);
    d.scrim.dataset.automationId = 'templates';
    this.grid = el('<div class="fx-eg-tpls fx-ptpl-grid" data-automation-id="templates.grid"></div>');
    d.body.appendChild(this.grid);
    d.button('Save Current Project as Template...', 'ghost', () => {
      void this.ctx.invoke('templates.saveAs').then((r) => {
        if (!r) return;
        this.painted.clear();
        void this.load();
      });
    });
    d.button('Close', 'primary', () => d.close());
    this.dialog = d;
  }

  private async load(): Promise<void> {
    const grid = this.grid as HTMLElement;
    const rows = (await this.ctx.run<TemplateRow[]>('templates.list', {}, true)) ?? [];
    patchList(grid, rows, {
      key: (t) => t.id,
      create: () => {
        const n = el(`<button class="fx-eg-tpl fx-ptpl" type="button" data-automation-id="templates.card">
          <span class="fx-eg-poster"><canvas width="256" height="144"></canvas></span>
          <span class="fx-eg-tpl-name"></span>
          <span class="fx-eg-tpl-meta"></span>
        </button>`);
        n.addEventListener('click', () => void this.create(String(n.dataset.key)));
        n.addEventListener('contextmenu', (e) => {
          e.preventDefault();
          const id = String(n.dataset.key);
          contextMenu(e.clientX, e.clientY, [
            { label: 'New Project', run: () => void this.create(id) },
            { label: 'Delete Template', enabled: id.startsWith('user/'), run: () => void this.ctx.run('templates.delete', { id }).then(() => this.load()) },
          ]);
        });
        return n;
      },
      update: (n, t) => {
        setText(q(n, '.fx-eg-tpl-name'), t.name);
        const meta = t.error ? t.error : `${t.category ?? 'User'} · ${t.width ?? 0} x ${t.height ?? 0} · ${(t.duration ?? 0).toFixed(1)} s`;
        setText(q(n, '.fx-eg-tpl-meta'), meta);
        setTitle(n, t.description || t.name);
      },
    });
    const pass = ++this.pass;
    for (const t of rows) {
      if (t.error || this.painted.has(t.id)) continue;
      const th = await this.ctx.run<Thumb>('templates.thumbnail', { id: t.id }, true);
      if (pass !== this.pass || !th) return;
      const card = grid.querySelector<HTMLElement>(`[data-key="${CSS.escape(t.id)}"]`);
      const canvas = card?.querySelector('canvas');
      if (!canvas) continue;
      paint(canvas, th);
      this.painted.add(t.id);
    }
  }

  private async create(id: string): Promise<void> {
    const r = await this.ctx.run('templates.create', { id });
    if (r) this.dialog?.close();
  }
}

function paint(canvas: HTMLCanvasElement, th: Thumb): void {
  const g = canvas.getContext('2d');
  if (!g) return;
  const img = g.createImageData(th.width, th.height);
  const n = th.width * th.height;
  for (let i = 0; i < n; i += 1) {
    const o = i * 6;
    img.data[i * 4] = parseInt(th.rgb.slice(o, o + 2), 16);
    img.data[i * 4 + 1] = parseInt(th.rgb.slice(o + 2, o + 4), 16);
    img.data[i * 4 + 2] = parseInt(th.rgb.slice(o + 4, o + 6), 16);
    img.data[i * 4 + 3] = 255;
  }
  g.putImageData(img, 0, 0);
}
