import { startApp } from './app/app';
import { loadWindowKey } from './engine/window-key';
import { startExtended } from './ui/extended';
import { startLanguage, startPageTranslator } from './ui/i18n';

void Promise.allSettled([startLanguage(), loadWindowKey()]).finally(() => {
  if (new URLSearchParams(location.search).has('extended')) startExtended();
  else startApp();
  startPageTranslator();
});
