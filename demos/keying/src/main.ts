import './demo.css';
import { startApp } from './app';
import { installKeying } from './engine/keying';
import { installPathMasks } from './engine/roto';

installKeying();
installPathMasks();
startApp();
