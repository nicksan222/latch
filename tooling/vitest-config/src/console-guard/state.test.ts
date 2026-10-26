import { describe, expect, test } from 'vite-plus/test';
import { expectConsole, finishFile, finishTest, startTest } from './state';

// Runs `body` as a nested test, then reopens the outer test the live guard is watching.
const nested = (body: () => void) => () => {
  startTest();
  try {
    body();
    finishTest();
  } finally {
    startTest();
  }
};

describe('console guard', () => {
  test('hides declared output', () => {
    expect(
      nested(() => {
        expectConsole('error', /expected failure/);
        expectConsole('warn', 'retrying');
        console.error('an expected failure: %s', 'quota');
        console.error('another expected failure');
        console.warn('retrying in 1s');
      })
    ).not.toThrow();
  });

  test('fails on undeclared output', () => {
    expect(nested(() => console.warn('surprise %d', 1))).toThrow(
      /Unexpected console output[^\n]*\n {2}console\.warn: surprise 1/
    );
  });

  test('matches the level as well as the text', () => {
    expect(
      nested(() => {
        expectConsole('warn', 'disk');
        console.error('disk full');
      })
    ).toThrow(/console\.error: disk full[\s\S]*console\.warn matching disk was never logged/);
  });

  test('fails when declared output never appears', () => {
    expect(nested(() => expectConsole('info', /ready/))).toThrow(
      'Expected console.info matching /ready/ was never logged'
    );
  });

  test('fails the file on output between tests', () => {
    startTest();
    finishTest();
    console.log('after the test ended');
    expect(() => expectConsole('log', 'late')).toThrow('inside a test or a beforeEach hook');
    startTest();
    expect(finishFile).toThrow(/outside a test[\s\S]*console\.log: after the test ended/);
  });
});
