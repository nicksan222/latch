import { AnchoredToastProvider, ToastProvider } from '@latch/ui/toast';
import { TooltipProvider } from '@latch/ui/tooltip';
import { QueryClientProvider, type QueryClient } from '@tanstack/react-query';
import { RouterProvider } from '@tanstack/react-router';
import { useEffect, useState } from 'react';
import { createQueryClient } from '~/lib/api/queries';
import { installDesktopEvents } from '~/lib/desktop-events';
import { createAppRouter } from './router';

export function App({
  queryClient,
  router
}: {
  queryClient?: QueryClient;
  router?: ReturnType<typeof createAppRouter>;
}) {
  const [client] = useState(() => queryClient ?? createQueryClient());
  const [appRouter] = useState(() => router ?? createAppRouter());
  // Desktop shell hooks (global shortcut → palette, no context menu in release).
  useEffect(() => installDesktopEvents(), []);
  return (
    <QueryClientProvider client={client}>
      <TooltipProvider delay={400}>
        <ToastProvider position="bottom-right">
          <AnchoredToastProvider>
            <RouterProvider router={appRouter} />
          </AnchoredToastProvider>
        </ToastProvider>
      </TooltipProvider>
    </QueryClientProvider>
  );
}
