// Keep installer decisions testable without touching a repository or global config.
export function assertHookPath(current) {
  if (current && current !== 'tooling/hooks') {
    throw new Error(
      'core.hooksPath is already configured differently; inspect git config --show-origin --get core.hooksPath before opting in.'
    );
  }
  return 'tooling/hooks';
}
