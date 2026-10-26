import { render, screen } from '@testing-library/react';
import { SettingsSection } from './page';

test('settings sections are named regions with their own heading', () => {
  render(
    <>
      <SettingsSection title="Security">
        <button>Change passphrase</button>
      </SettingsSection>
      <SettingsSection title="Appearance">
        <button>Theme</button>
      </SettingsSection>
    </>
  );
  const security = screen.getByRole('region', { name: 'Security' });
  expect(security).toContainElement(screen.getByRole('heading', { level: 2, name: 'Security' }));
  expect(security).toContainElement(screen.getByRole('button', { name: 'Change passphrase' }));
  expect(screen.getByRole('region', { name: 'Appearance' })).not.toBe(security);
});
