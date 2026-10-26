/** A recorded command and its public, already-sanitized output. No command execution. */
export interface TerminalStep {
  command: string;
  output: readonly string[];
}
export interface TerminalLine {
  id: string;
  kind: 'command' | 'output';
  text: string;
  pending: boolean;
}
export interface TerminalFrame {
  lines: TerminalLine[];
  complete: boolean;
  step: number;
}
const CHARACTER_MS = 30;
const OUTPUT_MS = 220;
const HOLD_MS = 650;
export function terminalDuration(steps: readonly TerminalStep[]): number {
  return steps.reduce(
    (sum, step) =>
      sum + step.command.length * CHARACTER_MS + step.output.length * OUTPUT_MS + HOLD_MS,
    0
  );
}
export function terminalFrame(steps: readonly TerminalStep[], elapsedMs: number): TerminalFrame {
  let elapsed = Math.max(0, elapsedMs);
  const lines: TerminalLine[] = [];
  for (let index = 0; index < steps.length; index++) {
    const step = steps[index]!;
    const typing = step.command.length * CHARACTER_MS;
    const commandComplete = elapsed >= typing;
    lines.push({
      id: `${index}:command`,
      kind: 'command',
      text: step.command.slice(0, Math.floor(elapsed / CHARACTER_MS)),
      pending: !commandComplete
    });
    if (!commandComplete) return { lines, complete: false, step: index };
    elapsed -= typing;
    for (let line = 0; line < step.output.length; line++) {
      if (elapsed < OUTPUT_MS) return { lines, complete: false, step: index };
      elapsed -= OUTPUT_MS;
      lines.push({
        id: `${index}:output:${line}`,
        kind: 'output',
        text: step.output[line]!,
        pending: false
      });
    }
    if (elapsed < HOLD_MS) return { lines, complete: false, step: index };
    elapsed -= HOLD_MS;
  }
  return { lines, complete: true, step: Math.max(0, steps.length - 1) };
}
