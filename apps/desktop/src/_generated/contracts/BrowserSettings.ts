// Generated from the Protobuf descriptor. Do not edit.
import type { AutofillScope } from './AutofillScope';
import type { Bookmark } from './Bookmark';
import type { BrowserEngine } from './BrowserEngine';

export type BrowserSettings = { "engine": BrowserEngine, "binary": string | null, "startPages": Array<string>, "bookmarks": Array<Bookmark>, "extensionsAllowed": boolean, "autofill": AutofillScope, "autofillEntryIds": Array<string>, };
