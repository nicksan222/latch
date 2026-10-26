import * as React from 'react';
import { useStableCallback } from './use-stable-callback';

/**
 * Copies NON-SECRET text (error messages, CLI commands, public keys, the
 * one-time recovery kit) with the web clipboard API. Vault secrets must go
 * through the `clipboard.copy` RPC instead so they never reach the webview.
 */
export async function writeTextToClipboard(value: string, target = 'text'): Promise<boolean> {
  if (typeof navigator === 'undefined' || !navigator.clipboard?.writeText) {
    throw new Error(`Clipboard API is unavailable while copying ${target}.`);
  }
  try {
    await navigator.clipboard.writeText(value);
    return true;
  } catch (cause) {
    throw new Error(`Failed to copy ${target} to the clipboard.`, { cause });
  }
}

export function useCopyToClipboard<TContext = void>({
  timeout = 2000,
  target = 'text',
  onCopy,
  onError
}: {
  timeout?: number;
  target?: string;
  onCopy?: (ctx: TContext) => void;
  onError?: (error: Error, ctx: TContext) => void;
} = {}): { copyToClipboard: (value: string, ctx: TContext) => void; isCopied: boolean } {
  const [isCopied, setIsCopied] = React.useState(false);
  const timeoutIdRef = React.useRef<ReturnType<typeof setTimeout> | null>(null);
  const copyToClipboard = useStableCallback((value: string, ctx: TContext): void => {
    void writeTextToClipboard(value, target).then(
      () => {
        if (timeoutIdRef.current) clearTimeout(timeoutIdRef.current);
        setIsCopied(true);
        onCopy?.(ctx);
        if (timeout !== 0) {
          timeoutIdRef.current = setTimeout(() => {
            setIsCopied(false);
            timeoutIdRef.current = null;
          }, timeout);
        }
      },
      (error: Error) => {
        console.error(error);
        onError?.(error, ctx);
      }
    );
  });

  React.useEffect(
    () => () => {
      if (timeoutIdRef.current) clearTimeout(timeoutIdRef.current);
    },
    []
  );

  return { copyToClipboard, isCopied };
}
