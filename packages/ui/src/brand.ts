/** Two offset hooked L forms with a 2px interlocking clearance. */
export const LATCH_MARK_POLYGONS = [
  [
    [4, 4],
    [24, 4],
    [24, 10],
    [10, 10],
    [10, 20],
    [4, 20]
  ],
  [
    [28, 28],
    [8, 28],
    [8, 22],
    [22, 22],
    [22, 12],
    [28, 12]
  ]
] as const;

export const LATCH_MARK_PATHS = LATCH_MARK_POLYGONS.map(
  (polygon) => `M${polygon.map(([x, y]) => `${x} ${y}`).join('L')}Z`
);

export function brandSvg({ color = '#3568e8', background = 'none' } = {}): string {
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"><rect width="32" height="32" rx="7" fill="${background}"/>${LATCH_MARK_PATHS.map((d) => `<path d="${d}" fill="${color}"/>`).join('')}</svg>`;
}
