import { describe, expect, it } from 'vitest';
import { terminalDuration, terminalFrame } from './terminal-playback';
const steps = [
  { command: 'latch ls acme', output: ['acme/runtime [env] 2 vars'] },
  { command: 'latch lock', output: ['locked'] }
];
describe('terminal playback', () => {
  it('types commands before revealing their output', () => {
    expect(terminalFrame(steps, 0).lines).toEqual([
      { id: '0:command', kind: 'command', text: '', pending: true }
    ]);
    const frame = terminalFrame(steps, 90);
    expect(frame.lines[0]?.text).toBe('lat');
    expect(frame.lines.some((line) => line.kind === 'output')).toBe(false);
    const afterCommand = terminalFrame(steps, steps[0]!.command.length * 30 + 220);
    expect(afterCommand.lines[1]?.text).toBe('acme/runtime [env] 2 vars');
  });
  it('finishes all steps once and gives a stable full transcript', () => {
    const full = terminalFrame(steps, terminalDuration(steps));
    expect(full.complete).toBe(true);
    expect(full.lines.map((line) => line.text)).toEqual([
      'latch ls acme',
      'acme/runtime [env] 2 vars',
      'latch lock',
      'locked'
    ]);
    expect(terminalFrame(steps, Number.MAX_SAFE_INTEGER)).toEqual(full);
  });
  it('handles empty steps and prevents negative clocks from leaking output', () => {
    expect(terminalDuration([])).toBe(0);
    expect(terminalFrame([], 0)).toEqual({ lines: [], complete: true, step: 0 });
    expect(terminalFrame(steps, -1)).toEqual(terminalFrame(steps, 0));
  });
});
