import type { LucideIcon, LucideProps } from 'lucide-react';

export interface MorphIconProps extends LucideProps {
  icon: LucideIcon;
  /** Kept for API compatibility with t3code's morphicons wrapper. */
  reducedMotion?: 'user' | 'always' | 'never';
}

/**
 * t3code animates icon swaps with `morphicons`. Latch keeps the call sites but
 * renders the lucide icon directly (no extra dependency, reduced-motion safe).
 */
export function MorphIcon({ icon: Icon, reducedMotion: _reducedMotion, ...props }: MorphIconProps) {
  return <Icon {...props} />;
}
