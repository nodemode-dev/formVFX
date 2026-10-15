import type { Ctx } from '../app/ctx';
import type { Panel } from '../ui/dock';
import { el, patchList, q, setClass, setDisabled, setHidden, setText } from '../ui/dom';
import { icon } from '../ui/icons';

interface Tutorial {
  id: string;
  title: string;
  summary: string;
  minutes: number;
  steps: number;
}

interface LearnState {
  tutorial: { id: string; title: string };
  step: number;
  steps: number;
  done: boolean;
  completed?: boolean[];
  current: { title: string; text: string; target?: string };
}

export class LearnPanel implements Panel {
  readonly id = 'learn';
  readonly title = 'Learn';
  readonly el: HTMLElement;
  private tutorials: Tutorial[] = [];
  private state: LearnState | null = null;
  private loaded = false;

  constructor(private readonly ctx: Ctx) {
    this.el = el(`<div class="fx-lp">
      <div class="fx-term-bar"><span class="fx-term-title">${icon('play', 14)}<span>Learn</span></span></div>
      <section class="fx-lp-active" hidden data-automation-id="learn.active">
        <div class="fx-lp-title"></div>
        <div class="fx-lp-steps"></div>
        <div class="fx-lp-step"></div>
        <div class="fx-lp-text"></div>
        <div class="fx-lp-actions">
          <button class="cv-btn" type="button" data-act="stop">Close Tutorial</button>
          <span class="fx-flex"></span>
          <button class="cv-btn" type="button" data-act="back">Back</button>
          <button class="cv-btn" type="button" data-act="showMe">Show Me</button>
          <button class="cv-btn cv-btn-primary" type="button" data-act="next">Next</button>
        </div>
      </section>
      <div class="fx-lp-rows" data-automation-id="learn.rows"></div>
    </div>`);
    for (const act of ['back', 'showMe', 'next'] as const) {
      q(this.el, `[data-act="${act}"]`).addEventListener('click', () => void this.ctx.run('learn.step', { action: act }));
    }
    q(this.el, '[data-act="stop"]').addEventListener('click', () => void this.ctx.run('learn.stop', {}));
    ctx.client.events.on('frontend', (ev) => {
      if (String(ev.command) !== 'learn.update') return;
      const p = ev.params as LearnState | Record<string, never> | undefined;
      this.state = p && 'tutorial' in p ? (p as LearnState) : null;
      this.render();
    });
  }

  shown(): void {
    setClass(document.body, 'fx-learn-docked', true);
    if (this.loaded) return;
    this.loaded = true;
    void this.load();
  }

  hidden(): void {
    setClass(document.body, 'fx-learn-docked', false);
  }

  private async load(): Promise<void> {
    try {
      const [list, st] = await Promise.all([
        this.ctx.client.exec<Tutorial[]>('learn.list', {}),
        this.ctx.client.exec<LearnState | null>('learn.state', {}),
      ]);
      this.tutorials = list ?? [];
      this.state = st && 'tutorial' in st ? st : null;
    } catch {
      this.tutorials = [];
    }
    this.render();
  }

  private render(): void {
    const st = this.state;
    const active = q(this.el, '.fx-lp-active');
    setHidden(active, !st);
    if (st) {
      setText(q(active, '.fx-lp-title'), st.tutorial.title);
      setText(q(active, '.fx-lp-step'), st.done ? 'Done' : `Step ${st.step + 1} of ${st.steps}: ${st.current.title}`);
      setText(q(active, '.fx-lp-text'), st.done ? 'You finished this tutorial. Close it, or pick another below.' : st.current.text);
      const dots = Array.from({ length: st.steps }, (_, i) => i);
      patchList(q(active, '.fx-lp-steps'), dots, {
        key: (i) => String(i),
        create: () => el('<span class="fx-lp-dot"></span>'),
        update: (n, i) => {
          setClass(n, 'is-done', !!st.completed?.[i] || st.done || i < st.step);
          setClass(n, 'is-current', !st.done && i === st.step);
        },
      });
      setDisabled(q<HTMLButtonElement>(active, '[data-act="back"]'), st.done || st.step === 0);
      setDisabled(q<HTMLButtonElement>(active, '[data-act="showMe"]'), st.done);
      setDisabled(q<HTMLButtonElement>(active, '[data-act="next"]'), st.done);
    }
    patchList(q(this.el, '.fx-lp-rows'), this.tutorials, {
      key: (t) => t.id,
      create: (t) => {
        const n = el('<button class="fx-lp-row" type="button" data-automation-id="learn.row"><span class="fx-lp-row-title"></span><span class="fx-lp-row-meta"></span><span class="fx-lp-row-summary"></span></button>');
        n.addEventListener('click', () => void this.ctx.run('learn.start', { id: t.id }));
        return n;
      },
      update: (n, t) => {
        setText(q(n, '.fx-lp-row-title'), t.title);
        setText(q(n, '.fx-lp-row-meta'), `${t.minutes} min · ${t.steps} steps`);
        setText(q(n, '.fx-lp-row-summary'), t.summary);
        setClass(n, 'is-active', st?.tutorial.id === t.id);
      },
    });
  }
}
