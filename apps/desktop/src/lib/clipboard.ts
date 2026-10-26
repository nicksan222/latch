import { isTauri } from '~/lib/api/transport';

/**
 * Writes NON-vault-secret text (CLI commands, public keys) to the clipboard.
 * Vault fields use `clipboard.copy`, whole env entries `env.copy`.
 */
export async function writeClipboardText(text: string): Promise<void> {
  if (isTauri()) {
    const { writeText } = await import('@tauri-apps/plugin-clipboard-manager');
    await writeText(text);
    return;
  }
  if (!navigator.clipboard?.writeText) throw new Error('Clipboard is unavailable');
  await navigator.clipboard.writeText(text);
}

/** Native file picker in Tauri; returns null in the browser (callers show a text field). */
export async function pickPath(options: {
  directory?: boolean;
  save?: boolean;
  title?: string;
  defaultPath?: string;
}): Promise<string | null> {
  if (!isTauri()) return null;
  const dialog = await import('@tauri-apps/plugin-dialog');
  if (options.save) {
    return (
      (await dialog.save({
        ...(options.title ? { title: options.title } : {}),
        ...(options.defaultPath ? { defaultPath: options.defaultPath } : {})
      })) ?? null
    );
  }
  const picked = await dialog.open({
    directory: options.directory ?? false,
    multiple: false,
    ...(options.title ? { title: options.title } : {})
  });
  return typeof picked === 'string' ? picked : null;
}
