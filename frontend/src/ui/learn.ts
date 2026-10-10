import type { Ctx } from '../app/ctx';
import { Dialog } from './dialogs';
import { el, patchList, q, setClass, setText } from './dom';

interface LearnState {
  tutorial: { id: string; title: string };
  step: number;
  steps: number;
  done: boolean;
  completed: boolean[];
  current: { title: string; text: string; target: string };
}

interface Tutorial {
  id: string;
  title: string;
  summary: string;
  minutes: number;
  steps: number;
}

export class LearnCoach {
  private card: HTMLElement | null = null;
  private list: Dialog | null = null;
  private lit: HTMLElement | null = null;

  constructor(private readonly ctx: Ctx) {}

  async openList(): Promise<void> {
    if (!this.list) {
      const d = new Dialog('In-App Tutorials', 520);
      d.scrim.dataset.automationId = 'learn';
      d.body.appendChild(el('<div class="fx-learn-list"></div>'));
      d.button('Close', 'primary', () => d.close());
      this.list = d;
    }
    const rows = (await this.ctx.run<Tutorial[]>('learn.list', {}, true)) ?? [];
    patchList(q(this.list.body, '.fx-learn-list'), rows, {
      key: (t) => t.id,
      create: () => {
        const n = el('<button class="fx-learn-item" type="button" data-automation-id="learn.tutorial"><span class="fx-learn-item-title"></span><span class="fx-note"></span></button>');
        n.addEventListener('click', () => {
          this.list?.close();
          void this.ctx.run('learn.start', { id: n.dataset.key });
        });
        return n;
      },
      update: (n, t) => {
        setText(q(n, '.fx-learn-item-title'), `${t.title} · ${t.minutes} min`);
        setText(q(n, '.fx-note'), t.summary);
      },
    });
    this.list.open();
  }

  private build(): HTMLElement {
    const c = el(`<div class="fx-learn" role="dialog" aria-live="polite" data-automation-id="learn.coach" hidden>
      <div class="fx-learn-head"><span class="fx-learn-tut"></span><span class="fx-learn-count"></span></div>
      <div class="fx-learn-title"></div>
      <p class="fx-learn-text"></p>
      <div class="fx-learn-dots"></div>
      <div class="fx-learn-actions">
        <button class="cv-btn" type="button" data-l="back">Back</button>
        <button class="cv-btn" type="button" data-l="showMe" data-automation-id="learn.showMe">Show Me</button>
        <button class="cv-btn cv-btn-primary" type="button" data-l="next">Next</button>
        <button class="cv-btn" type="button" data-l="stop">Close</button>
      </div>
    </div>`);
    for (const b of Array.from(c.querySelectorAll<HTMLElement>('[data-l]'))) {
      b.addEventListener('click', () => {
        const a = b.dataset.l;
        if (a === 'stop') void this.ctx.run('learn.stop');
        else void this.ctx.run('learn.step', { action: a });
      });
    }
    document.getElementById('overlay-root')?.appendChild(c);
    this.card = c;
    return c;
  }

  update(st: LearnState | null): void {
    const c = this.card ?? this.build();
    this.highlight(st ? st.current.target : null);
    if (!st) {
      c.hidden = true;
      return;
    }
    setText(q(c, '.fx-learn-tut'), st.tutorial.title);
    setText(q(c, '.fx-learn-count'), `${st.step + 1} of ${st.steps}`);
    setText(q(c, '.fx-learn-title'), st.done ? `${st.current.title} · done` : st.current.title);
    setText(q(c, '.fx-learn-text'), st.done ? 'Every step is done. Close the tutorial or start another from Help > In-App Tutorials.' : st.current.text);
    patchList(q(c, '.fx-learn-dots'), st.completed.map((d, i) => ({ d, i })), {
      key: (x) => String(x.i),
      create: () => el('<span class="fx-learn-dot"></span>'),
      update: (n, x) => {
        setClass(n, 'is-done', x.d);
        setClass(n, 'is-current', x.i === st.step);
      },
    });
    (q(c, '[data-l="back"]') as HTMLButtonElement).disabled = st.step === 0;
    (q(c, '[data-l="next"]') as HTMLButtonElement).disabled = st.step + 1 >= st.steps;
    c.hidden = false;
  }

  private highlight(target: string | null): void {
    const next = target
      ? document.querySelector<HTMLElement>(`[data-automation-id="${target}"]`) ?? document.querySelector<HTMLElement>(`[data-automation-id="panel.${target}"]`) ?? document.querySelector<HTMLElement>(`[data-automation-id="panel.tab.${target}"]`)
      : null;
    if (next === this.lit) return;
    this.lit?.classList.remove('fx-learn-target');
    next?.classList.add('fx-learn-target');
    this.lit = next;
  }
}
