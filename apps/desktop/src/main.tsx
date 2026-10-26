import { setTransport } from './lib/api/client';
import { isTauri } from './lib/api/transport';
import { mountApp } from './mount';
import { createTauriTransport } from './sdk/transport';

if (isTauri()) setTransport(createTauriTransport());

mountApp(document.getElementById('root')!);
