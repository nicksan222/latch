import assert from 'node:assert/strict';
import { readFileSync, writeFileSync, mkdtempSync, rmSync, existsSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { firefox } from 'playwright';

// A real runtime test, separate from browser UI fixtures. No installed browser fallback.
test(
  'managed Firefox imports native logins, autofills only matching origins, and reconciles one-way',
  { timeout: 90000 },
  async () => {
    const binary = process.env.LATCH_TEST_FIREFOX;
    assert.ok(binary, 'LATCH_TEST_FIREFOX must identify the disposable managed Firefox executable');
    const profile = mkdtempSync(join(tmpdir(), 'latch-firefox-native-'));
    const fixture =
      '<form action="/login" method="POST"><label>User<input name="username" autocomplete="username"></label><label>Password<input type="password" name="password" autocomplete="current-password"></label><button>Sign in</button></form>';
    const server = createServer((_req, res) => {
      res.setHeader('Content-Type', 'text/html; charset=utf-8');
      res.end(fixture);
    });
    await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
    const origin = `http://127.0.0.1:${server.address().port}`;
    let serial = 0;
    const seedRows = (credentials) => {
      const generation = `fixture-${++serial}`;
      writeFileSync(
        join(profile, 'latch-credentials.json'),
        JSON.stringify({ version: 1, generation, credentials }),
        { mode: 0o600 }
      );
      return generation;
    };
    const seed = (password) =>
      seedRows(
        password ? [{ id: 'fixture-login', origin, username: 'fixture-user', password }] : []
      );
    const waitAck = async (generation) => {
      const path = join(profile, 'latch-credentials-result.json');
      for (let i = 0; i < 200; i++) {
        if (existsSync(path)) {
          const result = JSON.parse(readFileSync(path));
          rmSync(path);
          assert.equal(result.generation, generation);
          return result;
        }
        await new Promise((resolve) => setTimeout(resolve, 100));
      }
      assert.fail('native password-manager import did not finish');
    };
    const waitResult = async (generation) => {
      const result = await waitAck(generation);
      assert.equal(result.ok, true, 'native credential synchronization failed');
      return result;
    };
    let browser;
    try {
      let generation = seed('fixture-password-one');
      browser = await firefox.launchPersistentContext(profile, {
        executablePath: binary,
        headless: process.env.LATCH_BROWSER_HEADLESS === '1',
        firefoxUserPrefs: { 'signon.autofillForms.http': true }
      });
      const result = await waitResult(generation);
      assert.equal(result.managed, 1);
      assert.equal(existsSync(join(profile, 'latch-credentials.json')), false);
      const page = await browser.newPage();
      await page.goto(origin);
      await page.locator('[name=username]').click();
      await assert.doesNotReject(() =>
        page.locator('[name=password]').evaluate(
          (el) =>
            new Promise((resolve, reject) => {
              const end = Date.now() + 10000;
              const check = () =>
                el.value === 'fixture-password-one'
                  ? resolve()
                  : Date.now() > end
                    ? reject(new Error('matching-origin autofill missing'))
                    : setTimeout(check, 100);
              check();
            })
        )
      );
      await page.goto(`http://localhost:${server.address().port}`);
      assert.equal(await page.locator('[name=password]').inputValue(), '');
      generation = seedRows([
        { id: 'fixture-login', origin, username: 'fixture-user', password: 'should-not-apply' },
        {
          id: 'invalid-login',
          origin: 'file:///not-an-origin',
          username: 'other',
          password: 'invalid'
        }
      ]);
      assert.equal((await waitAck(generation)).ok, false);
      await page.goto(origin);
      await page.locator('[name=username]').click();
      assert.equal(await page.locator('[name=password]').inputValue(), 'fixture-password-one');
      generation = seed('fixture-password-two');
      assert.equal((await waitResult(generation)).managed, 1);
      await page.goto(origin);
      await page.locator('[name=username]').click();
      await page.waitForFunction(
        () => document.querySelector('[name=password]').value === 'fixture-password-two'
      );
      await browser.close();
      browser = null;
      generation = seed('fixture-password-three');
      browser = await firefox.launchPersistentContext(profile, {
        executablePath: binary,
        headless: process.env.LATCH_BROWSER_HEADLESS === '1',
        firefoxUserPrefs: { 'signon.autofillForms.http': true }
      });
      assert.equal((await waitResult(generation)).managed, 1);
      const reopened = await browser.newPage();
      await reopened.goto(origin);
      await reopened.waitForFunction(
        () => document.querySelector('[name=password]').value === 'fixture-password-three'
      );
      generation = seed(null);
      assert.equal((await waitResult(generation)).managed, 0);
      await reopened.reload();
      assert.equal(await reopened.locator('[name=password]').inputValue(), '');
      // A later native-store failure must not strand earlier successful inserts.
      generation = seedRows([
        { id: 'partial-one', origin, username: 'partial-one', password: 'first-insert' },
        { id: 'partial-two', origin, username: 'partial-two', password: '\u0000' }
      ]);
      assert.equal((await waitAck(generation)).ok, false);
      generation = seedRows([
        { id: 'partial-one', origin, username: 'partial-one', password: 'recovered-insert' }
      ]);
      assert.equal((await waitResult(generation)).managed, 1);
      generation = seed(null);
      assert.equal((await waitResult(generation)).managed, 0);
      generation = seed('browser-owned-password');
      assert.equal((await waitResult(generation)).managed, 1);
      // Revoking Latch's GUID ownership models an existing browser-owned record.
      writeFileSync(join(profile, 'latch-login-ownership.json'), '[]', { mode: 0o600 });
      generation = seed('must-not-overwrite-browser-owned');
      assert.equal((await waitAck(generation)).ok, false);
      generation = seed(null);
      assert.equal((await waitResult(generation)).managed, 0);
      await reopened.reload();
      await reopened.locator('[name=username]').click();
      assert.equal(
        await reopened.locator('[name=password]').inputValue(),
        'browser-owned-password'
      );
    } finally {
      await browser?.close();
      await new Promise((resolve) => server.close(resolve));
      rmSync(profile, { recursive: true, force: true });
    }
  }
);
