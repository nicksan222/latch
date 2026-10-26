import { create } from '@bufbuild/protobuf';
import { AutofillScope, BrowserSettingsSchema, type BrowserSettings } from '@latch/rpc/v1';
import domainCatalog from '../_generated/domain-catalog.json';
export function defaultBrowserSettings(): BrowserSettings {
  return create(BrowserSettingsSchema, {
    startPages: [...domainCatalog.defaultBrowserSettings.startPages],
    bookmarks: [...domainCatalog.defaultBrowserSettings.bookmarks],
    autofillEntryIds: [...domainCatalog.defaultBrowserSettings.autofillEntryIds],
    autofill: AutofillScope.ALL
  });
}
export function basename(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}
