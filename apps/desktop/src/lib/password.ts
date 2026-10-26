import { domain } from './domain';
export const generatePassword = (length = 24) => domain('generate_password', length);
