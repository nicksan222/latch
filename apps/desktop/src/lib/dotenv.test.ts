import { afterEach, expect, it } from 'vite-plus/test';
import { setTransport } from '~/lib/api/client';
import { setupDomainResponse } from '~/test/helpers';
afterEach(() => setTransport(null));
import { formatEnv, parseDotenv } from './dotenv';

it('forwards dotenv source to Rust and returns its parsed wire result', async () => {
  const output = [{ key: 'API_TOKEN', value: 'x y', hidden: true, comment: null }];
  const call = setupDomainResponse({ operation: 'parse_dotenv', output });
  expect(await parseDotenv('API_TOKEN="x y"')).toEqual(output);
  expect(call).toHaveBeenCalledWith('domain.call', {
    operation: 'parse_dotenv',
    input: 'API_TOKEN="x y"'
  });
});
it('forwards the chosen export format and values unchanged', async () => {
  const call = setupDomainResponse({ operation: 'format_env', output: "export A='one'\n" });
  const vars = [{ key: 'A', value: 'one' }];
  expect(await formatEnv(vars, 'export')).toBe("export A='one'\n");
  expect(call).toHaveBeenCalledWith('domain.call', {
    operation: 'format_env',
    input: { vars, format: 'export' }
  });
});
