import { startApp } from './app/app';
import { startExtended } from './ui/extended';

if (new URLSearchParams(location.search).has('extended')) startExtended();
else startApp();
