export type ConsoleLevel = 'debug' | 'error' | 'info' | 'log' | 'trace' | 'warn';
export type ConsolePattern = RegExp | string;

type Expectation = { level: ConsoleLevel; pattern: ConsolePattern; seen: boolean };
type Output = { level: ConsoleLevel; message: string };
type GuardState = {
  expectations: Expectation[];
  inTest: boolean;
  patched: WeakSet<Console>;
  stray: Output[];
  unexpected: Output[];
};

const levels: readonly ConsoleLevel[] = ['debug', 'error', 'info', 'log', 'trace', 'warn'];
const hint = 'declare it with expectConsole() from @latch/vitest-config/console';

// Setup and test files can load separate copies of this module, so the state lives on the realm.
const realm = globalThis as { [key: symbol]: GuardState | undefined };
const state = (realm[Symbol.for('@latch/console-guard')] ??= {
  expectations: [],
  inTest: false,
  patched: new WeakSet(),
  stray: [],
  unexpected: []
});

/**
 * Declares console output the current test produces on purpose; matching output stays hidden.
 * Undeclared output fails the test, and so does a declaration nothing matches.
 */
export function expectConsole(level: ConsoleLevel, pattern: ConsolePattern): void {
  if (!state.inTest) {
    throw new Error('expectConsole() must be called inside a test or a beforeEach hook');
  }
  state.expectations.push({ level, pattern, seen: false });
}

function show(value: unknown): string {
  if (typeof value === 'string') return value;
  if (value instanceof Error) return value.stack ?? `${value.name}: ${value.message}`;
  try {
    return JSON.stringify(value) ?? String(value);
  } catch {
    return String(value);
  }
}

// A runtime-neutral `util.format`: the guard also runs in browser-mode tests.
function format([first, ...rest]: unknown[]): string {
  if (typeof first !== 'string') return [first, ...rest].map(show).join(' ');
  const head = first.replace(/%[sdifjoO%]/gu, (token) => {
    if (token === '%%') return '%';
    return rest.length > 0 ? show(rest.shift()) : token;
  });
  return [head, ...rest.map(show)].join(' ');
}

function capture(level: ConsoleLevel, args: unknown[]): void {
  const message = format(args);
  const declared = state.expectations.filter(
    ({ level: expected, pattern }) =>
      expected === level &&
      (typeof pattern === 'string' ? message.includes(pattern) : pattern.test(message))
  );
  for (const expectation of declared) expectation.seen = true;
  if (declared.length === 0) {
    (state.inTest ? state.unexpected : state.stray).push({ level, message });
  }
}

function report(title: string, outputs: Output[]): string[] {
  if (outputs.length === 0) return [];
  return [
    `${title} (${hint}):`,
    ...outputs.map(({ level, message }) => {
      const lines = message.split('\n');
      const more = lines.length > 4 ? '\n    …' : '';
      return `  console.${level}: ${lines.slice(0, 4).join('\n    ')}${more}`;
    })
  ];
}

function fail(problems: string[]): void {
  if (problems.length > 0) throw new Error(problems.join('\n'));
}

export function install(target: Console = console): void {
  if (state.patched.has(target)) return;
  state.patched.add(target);
  for (const level of levels) target[level] = (...args: unknown[]) => capture(level, args);
}

export function startTest(): void {
  state.expectations = [];
  state.unexpected = [];
  state.inTest = true;
}

export function finishTest(): void {
  const { expectations, unexpected } = state;
  state.expectations = [];
  state.unexpected = [];
  state.inTest = false;
  fail([
    ...report('Unexpected console output', unexpected),
    ...expectations
      .filter(({ seen }) => !seen)
      .map(
        ({ level, pattern }) =>
          `Expected console.${level} matching ${String(pattern)} was never logged`
      )
  ]);
}

export function finishFile(): void {
  const { stray } = state;
  state.stray = [];
  fail(report('Console output outside a test', stray));
}
