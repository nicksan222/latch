import { Theme } from '@latch/rpc/v1';
import { Button } from '@latch/ui/button';
import { ArrowLeftIcon, RotateCcwIcon } from 'lucide-react';
import { useEffect, useLayoutEffect, useState } from 'react';
import { App } from '../app/app';
import { createAppRouter } from '../app/router';
import { setTransport } from '../lib/api/client';
import { createQueryClient } from '../lib/api/queries';
import { useThemeStore } from '../stores/theme';
import { useUiStore } from '../stores/ui';
import { createDemoTransport } from './transport';
import './landing-demo.css';

/** The actual desktop app, in a separate document with a public, offline data source. */
function DemoSession() {
  const [session] = useState(() => ({
    client: createQueryClient(),
    router: createAppRouter({ memory: true, initialPath: '/p/acme/e/acme-login' }),
    transport: createDemoTransport()
  }));

  // Install the offline adapter before child query effects can request app data.
  useLayoutEffect(() => {
    const { client, transport } = session;
    setTransport(transport);
    const syncTheme = () => {
      const theme = document.documentElement.classList.contains('dark') ? Theme.DARK : Theme.LIGHT;
      transport.setTheme(theme);
      useThemeStore.getState().setTheme(theme);
      void client.invalidateQueries({ queryKey: ['settings'] });
    };
    syncTheme();
    window.addEventListener('storage', syncTheme);
    return () => {
      window.removeEventListener('storage', syncTheme);
      setTransport(null);
      client.clear();
      useUiStore.setState({ paletteOpen: false, addingVault: false, typeFilter: {} });
    };
  }, [session]);

  return (
    <>
      <Button
        className="demo-back"
        size="xs"
        variant="ghost"
        onClick={() => {
          const match = /^\/p\/([^/]+)/.exec(session.router.state.location.pathname);
          void session.router.navigate({
            to: '/p/$profileId',
            params: { profileId: match?.[1] ?? 'acme' }
          });
        }}
      >
        <ArrowLeftIcon /> All entries
      </Button>
      <div className="demo-app">
        <App queryClient={session.client} router={session.router} />
      </div>
    </>
  );
}

export function LandingDemo() {
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    if (window.parent === window) return;
    // This preview is embedded only by the same-origin landing document.
    // Keep gestures over its iframe scrolling that document instead of trapping them.
    const parent = window.parent;
    try {
      if (parent.location.origin !== window.location.origin) return;
    } catch {
      return;
    }
    const wheel = (event: WheelEvent) => {
      if (event.ctrlKey) return;
      event.preventDefault();
      const unit = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? parent.innerHeight : 1;
      parent.scrollBy({ top: event.deltaY * unit, left: event.deltaX * unit, behavior: 'instant' });
    };
    let touchY: number | null = null;
    const touchStart = (event: TouchEvent) => {
      touchY = event.touches.length === 1 ? event.touches[0]!.clientY : null;
    };
    const touchMove = (event: TouchEvent) => {
      if (touchY === null || event.touches.length !== 1) return;
      const nextY = event.touches[0]!.clientY;
      event.preventDefault();
      parent.scrollBy({ top: touchY - nextY, behavior: 'instant' });
      touchY = nextY;
    };
    window.addEventListener('wheel', wheel, { passive: false });
    window.addEventListener('touchstart', touchStart, { passive: true });
    window.addEventListener('touchmove', touchMove, { passive: false });
    return () => {
      window.removeEventListener('wheel', wheel);
      window.removeEventListener('touchstart', touchStart);
      window.removeEventListener('touchmove', touchMove);
    };
  }, []);

  return (
    <div className="landing-demo" aria-label="Interactive Latch demo">
      <div className="demo-disclosure">
        <span>Live app preview · fictional data · changes are not saved</span>
        <Button size="xs" variant="ghost" onClick={() => setRevision((value) => value + 1)}>
          <RotateCcwIcon /> Reset demo
        </Button>
      </div>
      <DemoSession key={revision} />
    </div>
  );
}
