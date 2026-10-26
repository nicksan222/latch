import { Select, SelectItem, SelectPopup, SelectTrigger, SelectValue } from '@latch/ui/select';
import type { ReactNode } from 'react';

export interface SelectOption<V extends string> {
  value: V;
  label: ReactNode;
}

/** Thin wrapper over coss Select for the common value/options case. */
export function SimpleSelect<V extends string>({
  value,
  onValueChange,
  options,
  id,
  className,
  size,
  disabled,
  placeholder,
  'aria-label': ariaLabel
}: {
  value: V | null;
  onValueChange: (value: V) => void;
  options: readonly SelectOption<V>[];
  id?: string;
  className?: string;
  size?: 'default' | 'sm' | 'xs' | 'compact' | 'lg';
  disabled?: boolean;
  placeholder?: string;
  'aria-label'?: string;
}) {
  return (
    <Select
      disabled={disabled ?? false}
      items={options}
      value={value}
      onValueChange={(v) => {
        if (v !== null) onValueChange(v as V);
      }}
    >
      <SelectTrigger aria-label={ariaLabel} className={className} id={id} size={size ?? 'default'}>
        <SelectValue placeholder={placeholder} />
      </SelectTrigger>
      <SelectPopup>
        {options.map((o) => (
          <SelectItem key={o.value} value={o.value}>
            {o.label}
          </SelectItem>
        ))}
      </SelectPopup>
    </Select>
  );
}
