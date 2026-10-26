import type { EnvFormat, EnvPair } from '../_generated/contracts';
import { domain } from './domain';
export const parseDotenv = (text: string) => domain('parse_dotenv', text);
export const formatEnv = (vars: readonly EnvPair[], format: EnvFormat) =>
  domain('format_env', { vars: [...vars], format });
