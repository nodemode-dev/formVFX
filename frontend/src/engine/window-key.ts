const HEADER = 'X-Formvfx-Window-Key';
const STORE = 'formvfx-window-key';

let key = '';

function remembered(): string {
  try {
    return sessionStorage.getItem(STORE) ?? '';
  } catch {
    return '';
  }
}

function remember(value: string): void {
  try {
    sessionStorage.setItem(STORE, value);
  } catch {
    return;
  }
}

export async function loadWindowKey(): Promise<void> {
  const holder = window as unknown as { __FORMVFX_WINDOW_KEY?: unknown };
  const given = holder.__FORMVFX_WINDOW_KEY;
  delete holder.__FORMVFX_WINDOW_KEY;
  if (typeof given === 'string' && given) {
    key = given;
    return;
  }
  try {
    const res = await fetch('/api/window/key', { method: 'POST', headers: { 'x-formvfx-local': '1' } });
    const body = res.ok ? ((await res.json()) as { key?: unknown }) : {};
    if (typeof body.key === 'string' && body.key) {
      key = body.key;
      remember(key);
      return;
    }
  } catch {
    key = '';
  }
  key = remembered();
}

export const windowKey = (): string => key;

export const keyHeaders = (): Record<string, string> => (key ? { [HEADER]: key } : {});
