import { useState } from 'react';

type Row<T> = { id: string; value: T };

function row<T>(value: T): Row<T> {
  return { id: crypto.randomUUID(), value };
}

/** Editor identity stays local: callers receive only the original wire values. */
export function useKeyedRows<T>(values: T[], onChange: (values: T[]) => void) {
  const [snapshot, setSnapshot] = useState(() => ({ values, rows: values.map(row) }));
  let rows = snapshot.rows;
  if (snapshot.values !== values) {
    // External replacements (e.g. imports) keep identity for retained values,
    // including duplicate values, without retaining removed plaintext rows.
    const available = new Map<T, Row<T>[]>();
    for (const existing of rows) {
      const matches = available.get(existing.value) ?? [];
      matches.push(existing);
      available.set(existing.value, matches);
    }
    rows = values.map((value) => available.get(value)?.shift() ?? row(value));
    setSnapshot({ values, rows });
  }
  const change = (next: Row<T>[]) => {
    const nextValues = next.map((item) => item.value);
    setSnapshot({ values: nextValues, rows: next });
    onChange(nextValues);
  };
  return {
    rows,
    append: (value: T) => change([...rows, row(value)]),
    update: (index: number, value: T) =>
      change(rows.map((item, i) => (i === index ? { ...item, value } : item))),
    remove: (index: number) => change(rows.filter((_, i) => i !== index)),
    move: (from: number, to: number) => {
      if (from < 0 || to < 0 || from >= rows.length || to >= rows.length) return;
      const next = [...rows];
      const [item] = next.splice(from, 1);
      next.splice(to, 0, item!);
      change(next);
    }
  };
}
