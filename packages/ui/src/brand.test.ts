import { describe, expect, it } from 'vitest';
import { brandSvg, LATCH_MARK_PATHS, LATCH_MARK_POLYGONS } from './brand';

describe('Latch brand mark', () => {
  it('exports the same two canonical paths used by SVG consumers', () => {
    expect(LATCH_MARK_POLYGONS).toHaveLength(2);
    expect(LATCH_MARK_PATHS).toHaveLength(2);
    expect(LATCH_MARK_PATHS).toEqual([
      'M4 4L24 4L24 10L10 10L10 20L4 20Z',
      'M28 28L8 28L8 22L22 22L22 12L28 12Z'
    ]);
    expect(brandSvg()).toContain(`<path d="${LATCH_MARK_PATHS[0]}" fill="#3568e8"/>`);
    expect(brandSvg({ color: 'currentColor', background: 'none' })).toContain(
      'fill="currentColor"'
    );
  });
});
