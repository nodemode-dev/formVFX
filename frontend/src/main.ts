import { startApp } from './app/app';
import { startExtended } from './ui/extended';
import { startLanguage, startPageTranslator } from './ui/i18n';

void startLanguage().finally(() => {
  if (new URLSearchParams(location.search).has('extended')) startExtended();
  else startApp();
  startPageTranslator();
});
