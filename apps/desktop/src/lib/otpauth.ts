import { domain } from './domain';
export const parseOtpauth = (uri: string) =>
  domain('parseOtpauth', { value: uri }).then((result) => result.value);
