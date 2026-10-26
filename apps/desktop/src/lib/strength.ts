import { useAsyncValue } from './async-ui';
import { domain } from './domain';
export const estimateStrength = (passphrase: string) => domain('estimate_strength', passphrase);
export function useStrength(passphrase: string) {
  return useAsyncValue(passphrase, estimateStrength);
}
