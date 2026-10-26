import { create } from 'zustand';
import type { EntryType } from '../_generated/contracts';

interface UiState {
  paletteOpen: boolean;
  setPaletteOpen: (open: boolean) => void;
  /** Show onboarding even though a vault exists ("Add vault" from the lock screen). */
  addingVault: boolean;
  setAddingVault: (adding: boolean) => void;
  /** Per-profile entry list type filter. */
  typeFilter: Record<string, EntryType | null>;
  setTypeFilter: (profileId: string, type: EntryType | null) => void;
}

export const useUiStore = create<UiState>((set) => ({
  paletteOpen: false,
  setPaletteOpen: (paletteOpen) => set({ paletteOpen }),
  addingVault: false,
  setAddingVault: (addingVault) => set({ addingVault }),
  typeFilter: {},
  setTypeFilter: (profileId, type) =>
    set((s) => ({ typeFilter: { ...s.typeFilter, [profileId]: type } }))
}));
