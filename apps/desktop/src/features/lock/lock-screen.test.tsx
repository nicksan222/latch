import { LatchAgent } from '@latch/rpc/v1';
import type { VaultStatus } from '@latch/rpc/v1';
import { cleanup, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, expect, it, vi } from 'vite-plus/test';
import { renderWithProviders, setupFixture } from '~/test/helpers';
import { LockScreen } from './lock-screen';

let backend: Awaited<ReturnType<typeof setupFixture>>;
afterEach(async () => {
  cleanup();
  await backend.dispose();
});

it('associates identity unlock help and failure with the identity input', async () => {
  backend = await setupFixture({ unlocked: false });
  const status = (await backend.handle(LatchAgent.method.vaultStatus, undefined)) as VaultStatus;
  const request = backend.requestBytes.bind(backend);
  vi.spyOn(backend, 'requestBytes').mockImplementation(async (method, payload) => {
    if (method === LatchAgent.method.vaultUnlock)
      throw new Error('Identity could not unlock vault');
    return request(method, payload);
  });
  renderWithProviders(<LockScreen status={status} />);
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: 'Use an identity file instead' }));
  const input = screen.getByRole('textbox', { name: 'Identity file' });
  expect(input).toHaveAccessibleDescription(
    'An age identity (recovery kit or YubiKey plugin) that is a vault recipient.'
  );
  await user.type(input, '/missing/identity.txt');
  await user.click(screen.getByRole('button', { name: 'Unlock' }));
  const error = await screen.findByRole('alert');
  const invalidInput = screen.getByRole('textbox', { name: 'Identity file' });
  expect(invalidInput).toHaveAttribute('aria-invalid', 'true');
  expect(invalidInput.getAttribute('aria-describedby')?.split(' ')).toContain(error.id);
});
