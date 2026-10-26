import type { EnvFormat, EnvPair } from '@latch/rpc/v1';
import { domain } from './domain';
export const parseDotenv = (text: string) =>
  domain('parseDotenv', { value: text }).then((result) => result.items);
export const formatEnv = (vars: readonly EnvPair[], format: EnvFormat) =>
  domain('formatEnv', { vars: [...vars], format }).then((result) => result.value);
