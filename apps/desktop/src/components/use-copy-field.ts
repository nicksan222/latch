import { useRpcMutation } from '~/lib/api/queries';
import { notify } from '~/lib/notify';

/**
 * Copies a vault field through `clipboard.copy`: the agent writes the value to
 * the OS clipboard and clears it later, so plaintext never enters the webview.
 */
export function useCopyField() {
  return useRpcMutation<'clipboard.copy', { id: string; field: string; label?: string }>(
    'clipboard.copy',
    {
      toParams: ({ id, field }) => ({ id, field }),
      onSuccess: (result, args) => {
        notify.success(
          `Copied ${args.label ?? 'value'}`,
          result.clearAfterSecs > 0 ? `Clipboard clears in ${result.clearAfterSecs}s` : undefined
        );
      },
      onError: (error, args) => notify.error(`Could not copy ${args.label ?? 'value'}`, error)
    }
  );
}
