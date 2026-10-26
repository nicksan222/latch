import { LatchAgent, EnvFormat } from '@latch/rpc/v1';
import { afterEach, expect, it } from 'vite-plus/test';
import { setTransport } from '~/lib/api/client';
import { setupDomainResponse } from '~/test/helpers';
afterEach(() => setTransport(null));
import { formatEnv, parseDotenv } from './dotenv';

it('forwards dotenv source to Rust and returns its parsed wire result', async () => {
  const output = [{ key: 'API_TOKEN', value: 'x y', hidden: true }];
  const call = setupDomainResponse({
    operation: { case: 'parseDotenv', value: { items: output } }
  });
  expect(await parseDotenv('API_TOKEN="x y"')).toMatchObject(output);
  expect(call).toHaveBeenCalledWith(
    LatchAgent.method.domainCall,
    expect.objectContaining({
      operation: expect.objectContaining({
        case: 'parseDotenv',
        value: expect.objectContaining({ value: 'API_TOKEN="x y"' })
      })
    })
  );
});
it('forwards the chosen export format and values unchanged', async () => {
  const call = setupDomainResponse({
    operation: { case: 'formatEnv', value: { value: "export A='one'\n" } }
  });
  const vars = [{ $typeName: 'latch.v1.EnvPair' as const, key: 'A', value: 'one' }];
  expect(await formatEnv(vars, EnvFormat.EXPORT)).toBe("export A='one'\n");
  expect(call).toHaveBeenCalledWith(
    LatchAgent.method.domainCall,
    expect.objectContaining({
      operation: expect.objectContaining({
        case: 'formatEnv',
        value: expect.objectContaining({ vars, format: EnvFormat.EXPORT })
      })
    })
  );
});
