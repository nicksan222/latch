import { domain } from './domain';
export const generatePassword = (length = 24) =>
  domain('generatePassword', { value: length }).then((result) => result.value);
