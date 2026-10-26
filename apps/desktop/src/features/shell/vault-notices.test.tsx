import { create } from '@bufbuild/protobuf';
import { VaultStatusSchema, VaultTrustState } from '@latch/rpc/v1';
import type { VaultStatus } from '@latch/rpc/v1';
import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vite-plus/test';
import { renderWithProviders } from '~/test/helpers';
import { VaultNoticesView } from './vault-notices';

const base: VaultStatus = create(VaultStatusSchema, {
  initialized: true,
  locked: false,
  vaultId: 'v',
  path: '/v',
  recipients: [],
  dirty: false,
  ahead: 0,
  behind: 0,
  hasRemote: false,
  lastSyncAt: undefined,
  locksInSecs: 600n,
  trust: {
    state: VaultTrustState.PINNED,
    fingerprint: 'ABCD-EFGH-JKMN-PQRS-TVWX-YZ23-4567-89AB',
    keySeq: 0n
  },
  integrityProblems: [],
  rotationPending: false,
  memoryLocked: true
});

describe('VaultNoticesView', () => {
  it('renders nothing for a pinned, healthy vault', () => {
    renderWithProviders(<VaultNoticesView status={base} onOpenDoctor={() => {}} />);
    expect(screen.queryByTestId('vault-notices')).toBeNull();
  });

  it('shows the fingerprint on first use and can be dismissed', async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <VaultNoticesView
        status={{ ...base, trust: { ...base.trust!, state: VaultTrustState.FIRST_USE } }}
        onOpenDoctor={() => {}}
      />
    );
    expect(screen.getByTestId('trust-banner')).toHaveTextContent(
      'ABCD-EFGH-JKMN-PQRS-TVWX-YZ23-4567-89AB'
    );
    await user.click(screen.getByRole('button', { name: 'Dismiss fingerprint notice' }));
    expect(screen.queryByTestId('trust-banner')).toBeNull();
  });

  it('links integrity problems to the doctor', async () => {
    const user = userEvent.setup();
    const open = vi.fn();
    renderWithProviders(
      <VaultNoticesView
        status={{
          ...base,
          integrityProblems: ['manifest.sig: vault manifest not signed by this vault']
        }}
        onOpenDoctor={open}
      />
    );
    expect(screen.getByTestId('integrity-banner')).toHaveTextContent('manifest.sig');
    await user.click(screen.getByRole('button', { name: 'Open doctor' }));
    expect(open).toHaveBeenCalled();
  });

  it('stays hidden while locked', () => {
    renderWithProviders(
      <VaultNoticesView
        status={{ ...base, locked: true, integrityProblems: ['x'] }}
        onOpenDoctor={() => {}}
      />
    );
    expect(screen.queryByTestId('vault-notices')).toBeNull();
  });
});
