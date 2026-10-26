import { act, renderHook } from '@testing-library/react';
import { useState } from 'react';
import { describe, expect, it } from 'vite-plus/test';
import { useKeyedRows } from './use-keyed-rows';

function useEditor<T>(initial: T[]) {
  const [values, setValues] = useState(initial);
  return { ...useKeyedRows(values, setValues), values, replace: setValues };
}

describe('useKeyedRows', () => {
  it('preserves duplicate row identities across edits, deletion and insertion without wire IDs', () => {
    const { result } = renderHook(() => useEditor(['', '', 'third']));
    const ids = result.current.rows.map((row) => row.id);
    expect(new Set(ids).size).toBe(3);
    act(() => result.current.update(1, 'edited'));
    expect(result.current.rows.map((row) => row.id)).toEqual(ids);
    act(() => result.current.remove(0));
    expect(result.current.rows.map((row) => row.id)).toEqual(ids.slice(1));
    act(() => result.current.append(''));
    expect(result.current.rows[2]!.id).not.toBe(ids[0]);
    expect(result.current.values).toEqual(['edited', 'third', '']);
  });

  it('moves row identity with the value and retains unchanged rows after external imports', () => {
    const first = { key: 'ONE', value: 'first' };
    const second = { key: 'TWO', value: 'second' };
    const { result } = renderHook(() => useEditor([first, second]));
    const ids = result.current.rows.map((row) => row.id);
    act(() => result.current.move(1, 0));
    expect(result.current.rows.map((row) => row.id)).toEqual([ids[1], ids[0]]);
    act(() => result.current.replace([first, { key: 'NEW', value: 'imported' }]));
    expect(result.current.rows[0]!.id).toBe(ids[0]);
    expect(result.current.rows[1]!.id).not.toBe(ids[1]);
    expect(result.current.values).toEqual([first, { key: 'NEW', value: 'imported' }]);
    act(() => result.current.move(-1, 1));
    expect(result.current.values[0]).toBe(first);
  });
});
