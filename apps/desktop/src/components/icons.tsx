import { EntryType } from '@latch/rpc/v1';
import { cn } from '@latch/ui/utils';
import {
  BookOpenIcon,
  BoxIcon,
  BriefcaseIcon,
  BuildingIcon,
  CloudIcon,
  CodeIcon,
  DatabaseIcon,
  DownloadIcon,
  FileIcon,
  FlaskConicalIcon,
  FolderIcon,
  GamepadIcon,
  GlobeIcon,
  HeartIcon,
  HomeIcon,
  KeyRoundIcon,
  KeySquareIcon,
  LockKeyholeIcon,
  RocketIcon,
  ServerIcon,
  ShieldIcon,
  ShoppingCartIcon,
  StickyNoteIcon,
  TerminalIcon,
  UserIcon,
  VariableIcon,
  type LucideIcon
} from 'lucide-react';

export const ENTRY_TYPE_ICON: Record<EntryType, LucideIcon> = {
  [EntryType.UNSPECIFIED]: FileIcon,
  [EntryType.LOGIN]: GlobeIcon,
  [EntryType.SECRET]: LockKeyholeIcon,
  [EntryType.ENV]: VariableIcon,
  [EntryType.FILE]: FileIcon,
  [EntryType.NOTE]: StickyNoteIcon,
  [EntryType.SSH_KEY]: TerminalIcon,
  [EntryType.DATABASE]: DatabaseIcon,
  [EntryType.API_KEY]: KeyRoundIcon
};

/** Curated lucide icons for profiles (stored kebab-case in `Profile.icon`). */
export const PROFILE_ICONS: Record<string, LucideIcon> = {
  briefcase: BriefcaseIcon,
  rocket: RocketIcon,
  user: UserIcon,
  building: BuildingIcon,
  code: CodeIcon,
  globe: GlobeIcon,
  home: HomeIcon,
  shield: ShieldIcon,
  server: ServerIcon,
  cloud: CloudIcon,
  'flask-conical': FlaskConicalIcon,
  heart: HeartIcon,
  box: BoxIcon,
  'book-open': BookOpenIcon,
  gamepad: GamepadIcon,
  'shopping-cart': ShoppingCartIcon,
  'key-square': KeySquareIcon,
  download: DownloadIcon,
  folder: FolderIcon
};

export function EntryTypeIcon({ type, className }: { type: EntryType; className?: string }) {
  const Icon = ENTRY_TYPE_ICON[type] ?? FileIcon;
  return <Icon aria-hidden className={cn('size-4 shrink-0', className)} />;
}

/**
 * Profile avatar: icon on a tint of the profile color. The color is the one
 * hard-coded value allowed (AGENTS.md rule 6) and is applied through a CSS
 * variable on the element.
 */
export function ProfileAvatar({
  icon,
  color,
  size = 'sm',
  className
}: {
  icon: string;
  color: string;
  size?: 'xs' | 'sm' | 'md' | 'lg';
  className?: string;
}) {
  const Icon = PROFILE_ICONS[icon] ?? FolderIcon;
  return (
    <span
      aria-hidden
      className={cn(
        'inline-flex shrink-0 items-center justify-center rounded-md bg-[color-mix(in_srgb,var(--profile-color)_16%,transparent)] text-(--profile-color) dark:bg-[color-mix(in_srgb,var(--profile-color)_24%,transparent)] dark:text-[color-mix(in_srgb,var(--profile-color)_70%,white)]',
        size === 'xs' && 'size-4 rounded-sm [&_svg]:size-2.5',
        size === 'sm' && 'size-5 [&_svg]:size-3',
        size === 'md' && 'size-7 [&_svg]:size-4',
        size === 'lg' && 'size-10 rounded-lg [&_svg]:size-5',
        className
      )}
      style={{ '--profile-color': color } as React.CSSProperties}
    >
      <Icon />
    </span>
  );
}

export function ProfileDot({ color, className }: { color: string; className?: string }) {
  return (
    <span
      aria-hidden
      className={cn('inline-block size-2 shrink-0 rounded-full bg-(--profile-color)', className)}
      style={{ '--profile-color': color } as React.CSSProperties}
    />
  );
}
