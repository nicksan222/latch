import type { SVGProps } from 'react';
import { LATCH_MARK_PATHS } from '../brand';

export function LatchMark({ className, ...props }: SVGProps<SVGSVGElement>) {
  return (
    <svg
      viewBox="0 0 32 32"
      fill="currentColor"
      aria-hidden="true"
      className={className}
      {...props}
    >
      {LATCH_MARK_PATHS.map((d) => (
        <path key={d} d={d} />
      ))}
    </svg>
  );
}
