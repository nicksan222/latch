import type { BrowserSettings } from '../_generated/contracts';
import domainCatalog from '../_generated/contracts/domain-catalog.json';
export function defaultBrowserSettings(): BrowserSettings {
  return {
    ...structuredClone(domainCatalog.defaultBrowserSettings),
    engine: 'firefox',
    binary: null
  } as BrowserSettings;
}
export function basename(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}
