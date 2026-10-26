import {
  createHashHistory,
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  lazyRouteComponent
} from '@tanstack/react-router';
import { EntryDetail } from '~/features/entries/entry-detail';
import { NewEntry } from '~/features/entries/new-entry';
import { ProfileView, NoEntrySelected } from '~/features/entries/profile-view';
import { ImportExportPage } from '~/features/import-export/import-export-page';
import { ProfileEditor } from '~/features/profiles/profile-editor';
import { RecipientsPage } from '~/features/recipients/recipients-page';
import { SettingsPage } from '~/features/settings/settings-page';
import { HomePage } from '~/features/shell/home-page';
import { SyncPage } from '~/features/sync/sync-page';
import { TrashPage } from '~/features/trash/trash-page';
import { ENTRY_TYPES } from '~/lib/entries';
import type { EntryType } from '../_generated/contracts';
import { Gate } from './gate';

const rootRoute = createRootRoute({
  component: () => (
    <Gate>
      <Outlet />
    </Gate>
  )
});

const indexRoute = createRoute({ getParentRoute: () => rootRoute, path: '/', component: HomePage });

export const profileRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/p/$profileId',
  component: ProfileView
});

const profileIndexRoute = createRoute({
  getParentRoute: () => profileRoute,
  path: '/',
  component: NoEntrySelected
});

export const entryRoute = createRoute({
  getParentRoute: () => profileRoute,
  path: 'e/$entryId',
  component: EntryDetail
});

export const newEntryRoute = createRoute({
  getParentRoute: () => profileRoute,
  path: 'new/$type',
  params: {
    parse: (raw) => ({
      type: (ENTRY_TYPES.includes(raw.type as EntryType) ? raw.type : 'login') as EntryType
    }),
    stringify: (p) => ({ type: p.type })
  },
  component: NewEntry
});

export const profileEditRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/profile/$profileId',
  component: () => <ProfileEditor mode="edit" />
});

const profileNewRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/profiles/new',
  component: () => <ProfileEditor mode="create" />
});

const syncRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/sync',
  component: SyncPage
});
const recipientsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/recipients',
  component: RecipientsPage
});
const importExportRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/import-export',
  component: ImportExportPage
});
const settingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/settings',
  component: SettingsPage
});
const trashRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/trash',
  component: TrashPage
});

// Kept outside production bundles and the ordinary product navigation.
const developmentRoutes = import.meta.env.DEV
  ? [
      createRoute({
        getParentRoute: () => rootRoute,
        path: '/design-system',
        component: lazyRouteComponent(
          () => import('~/features/design-system/design-system-page'),
          'DesignSystemPage'
        )
      })
    ]
  : [];

const routeTree = rootRoute.addChildren([
  ...developmentRoutes,
  indexRoute,
  profileRoute.addChildren([profileIndexRoute, entryRoute, newEntryRoute]),
  profileEditRoute,
  profileNewRoute,
  syncRoute,
  recipientsRoute,
  importExportRoute,
  settingsRoute,
  trashRoute
]);

/** Hash history: Tauri serves static files, so the path must stay `/`. */
export function createAppRouter(options: { memory?: boolean; initialPath?: string } = {}) {
  return createRouter({
    routeTree,
    history: options.memory
      ? createMemoryHistory({ initialEntries: [options.initialPath ?? '/'] })
      : createHashHistory(),
    defaultPreload: false
  });
}

declare module '@tanstack/react-router' {
  interface Register {
    router: ReturnType<typeof createAppRouter>;
  }
}
