import { toastManager } from '@latch/ui/toast';
import { errorMessage } from '~/lib/api/errors';

/** Toast helpers on top of t3code's toast manager. */
export const notify = {
  success(title: string, description?: string) {
    toastManager.add({
      type: 'success',
      title,
      ...(description ? { description } : {}),
      timeout: 4000
    });
  },
  info(title: string, description?: string) {
    toastManager.add({
      type: 'info',
      title,
      ...(description ? { description } : {}),
      timeout: 4000
    });
  },
  warning(title: string, description?: string) {
    toastManager.add({
      type: 'warning',
      title,
      ...(description ? { description } : {}),
      timeout: 6000
    });
  },
  error(title: string, error?: unknown) {
    toastManager.add({
      type: 'error',
      title,
      ...(error !== undefined ? { description: errorMessage(error) } : {}),
      timeout: 8000
    });
  }
};
