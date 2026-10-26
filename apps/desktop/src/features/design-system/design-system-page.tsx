import { Theme } from '@latch/rpc/v1';
import { Alert, AlertDescription, AlertTitle } from '@latch/ui/alert';
import { Badge } from '@latch/ui/badge';
import { Button } from '@latch/ui/button';
import {
  Dialog,
  DialogClose,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogPopup,
  DialogTitle,
  DialogTrigger
} from '@latch/ui/dialog';
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@latch/ui/empty';
import { Skeleton } from '@latch/ui/skeleton';
import {
  AlertCircleIcon,
  CheckCircleIcon,
  FolderIcon,
  InfoIcon,
  LoaderCircleIcon,
  TriangleAlertIcon
} from 'lucide-react';
import { useState } from 'react';
import { FormField, Input, PasswordInput } from '~/components/fields';
import {
  ContentTitle,
  PageContainer,
  PageTitle,
  SettingsRow,
  SettingsSection
} from '~/components/page';
import { AppHeader } from '~/features/shell/app-header';
import { useSettings } from '~/lib/api/queries';
import { useThemeStore } from '~/stores/theme';

/** Development reference. Examples are local UI state and contain no vault values. */
export function DesignSystemPage() {
  const settings = useSettings();
  const theme = useThemeStore((s) => s.theme);
  const setTheme = useThemeStore((s) => s.setTheme);
  const [password, setPassword] = useState('demo-value-only');
  return (
    <>
      <AppHeader
        title={<PageTitle>Design system</PageTitle>}
        actions={<Badge variant="outline">Development only</Badge>}
      />
      <PageContainer width="wide">
        <div className="space-y-2">
          <ContentTitle>Compact by design</ContentTitle>
          <p className="max-w-prose text-sm text-muted-foreground">
            The shared vocabulary for Latch: neutral surfaces, blue actions, and clear feedback. All
            examples below use fictional, local data.
          </p>
        </div>
        <SettingsSection title="Appearance">
          <SettingsRow
            title="Theme"
            description="Inspect the same components in light, dark, or your system appearance."
            control={
              <div className="flex flex-wrap gap-1" role="group" aria-label="Reference theme">
                {([Theme.LIGHT, Theme.DARK, Theme.SYSTEM] as const).map((value) => (
                  <Button
                    key={value}
                    disabled={settings.isPending}
                    variant={theme === value ? 'secondary' : 'ghost'}
                    size="sm"
                    aria-pressed={theme === value}
                    onClick={() => setTheme(value)}
                  >
                    {value === Theme.LIGHT ? 'Light' : value === Theme.DARK ? 'Dark' : 'System'}
                  </Button>
                ))}
              </div>
            }
          />
        </SettingsSection>
        <SettingsSection title="Typography & surfaces">
          <SettingsRow
            title="Type roles"
            description="Keep workspace titles compact; use stronger headings for detail content."
          >
            <div className="grid gap-4 sm:grid-cols-2">
              <div className="space-y-2">
                <p className="font-medium text-sm">Workspace title · PageTitle</p>
                <ContentTitle>Content title</ContentTitle>
                <p className="text-sm">Body text and field labels support everyday tasks.</p>
                <p className="text-xs text-muted-foreground">
                  Helper text and metadata provide context.
                </p>
                <code className="block break-all font-mono text-xs">
                  git@example.test:demo/vault.git
                </code>
              </div>
              <div className="grid gap-2 text-sm">
                <div className="rounded-lg border bg-background p-3">
                  Canvas <span className="text-muted-foreground">/ background</span>
                </div>
                <div className="rounded-lg border bg-card p-3 text-card-foreground">
                  Card <span className="text-muted-foreground">/ grouped content</span>
                </div>
                <div className="rounded-lg border bg-muted p-3 text-muted-foreground">
                  Muted / secondary context
                </div>
                <div className="rounded-lg border bg-popover p-3 text-popover-foreground">
                  Popover / focused content
                </div>
              </div>
            </div>
          </SettingsRow>
        </SettingsSection>
        <SettingsSection title="Actions">
          <SettingsRow
            title="Button variants"
            description="Reserve the primary action for the next meaningful step. These are visual samples."
          >
            <div className="flex flex-wrap gap-2">
              <Button>Primary</Button>
              <Button variant="secondary">Secondary</Button>
              <Button variant="outline">Outline</Button>
              <Button variant="ghost">Ghost</Button>
              <Button variant="destructive">Destructive</Button>
              <Button variant="destructive-outline">Destructive outline</Button>
            </div>
          </SettingsRow>
          <SettingsRow
            title="Sizes & states"
            description="Pending actions keep their label and prevent repeated activation."
          >
            <div className="flex flex-wrap items-center gap-2">
              <Button size="compact" variant="outline">
                Compact
              </Button>
              <Button size="sm" variant="outline">
                Small
              </Button>
              <Button disabled>Disabled</Button>
              <Button disabled aria-busy="true">
                <LoaderCircleIcon className="motion-safe:animate-spin" />
                Saving…
              </Button>
            </div>
          </SettingsRow>
        </SettingsSection>
        <SettingsSection title="Fields">
          <div className="grid gap-5 p-4 sm:grid-cols-2">
            <FormField
              label="Profile name"
              description="A short, recognizable name for this demo profile."
            >
              {(props) => <Input {...props} defaultValue="Example project" />}
            </FormField>
            <FormField
              label="Remote URL"
              description="Use an SSH or HTTPS repository URL."
              error="Enter a valid Git URL."
            >
              {(props) => <Input {...props} defaultValue="invalid-example" />}
            </FormField>
            <FormField
              label="Demo password"
              description="Fictional input only. Reveal and edit it locally."
            >
              {(props) => <PasswordInput {...props} value={password} onChange={setPassword} />}
            </FormField>
            <FormField
              label="Unavailable field"
              description="Disabled controls remain visible but cannot be edited."
            >
              {(props) => <Input {...props} disabled value="Read-only example" />}
            </FormField>
          </div>
        </SettingsSection>
        <SettingsSection title="Status & feedback">
          <SettingsRow title="Badges" description="Pair state colors with explicit labels.">
            <div className="flex flex-wrap gap-2">
              <Badge variant="outline">Neutral</Badge>
              <Badge variant="info">Information</Badge>
              <Badge variant="success">Ready</Badge>
              <Badge variant="warning">Attention</Badge>
              <Badge variant="error">Error</Badge>
            </div>
          </SettingsRow>
          <div className="grid gap-3 p-4 sm:grid-cols-2">
            <Alert variant="info">
              <InfoIcon />
              <AlertTitle>Information</AlertTitle>
              <AlertDescription>This reference is available during development.</AlertDescription>
            </Alert>
            <Alert variant="success">
              <CheckCircleIcon />
              <AlertTitle>Example complete</AlertTitle>
              <AlertDescription>The local demonstration is ready.</AlertDescription>
            </Alert>
            <Alert variant="warning">
              <TriangleAlertIcon />
              <AlertTitle>Review before continuing</AlertTitle>
              <AlertDescription>Use warnings when an action needs extra care.</AlertDescription>
            </Alert>
            <Alert variant="error">
              <AlertCircleIcon />
              <AlertTitle>Example could not be saved</AlertTitle>
              <AlertDescription>Check the highlighted field and try again.</AlertDescription>
            </Alert>
          </div>
        </SettingsSection>
        <SettingsSection title="Empty & loading">
          <div className="grid sm:grid-cols-2 sm:divide-x divide-border/50">
            <Empty size="compact">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <FolderIcon />
                </EmptyMedia>
                <EmptyTitle>No demo entries</EmptyTitle>
                <EmptyDescription>
                  Empty states explain what belongs here and how to begin.
                </EmptyDescription>
              </EmptyHeader>
            </Empty>
            <div
              className="flex flex-col justify-center gap-4 p-6"
              role="status"
              aria-label="Loading demo entries"
              aria-busy="true"
            >
              <p className="text-sm text-muted-foreground">Loading entries…</p>
              <div className="space-y-3" aria-hidden="true">
                {[0, 1, 2].map((row) => (
                  <div className="flex items-center gap-3" key={row}>
                    <Skeleton className="size-8 shrink-0" />
                    <div className="flex-1 space-y-2">
                      <Skeleton className="h-3 w-2/3" />
                      <Skeleton className="h-3 w-1/3" />
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </SettingsSection>
        <SettingsSection title="Dialogs">
          <SettingsRow
            title="Focused confirmation"
            description="Open the example to inspect focus, dismissal, and action placement."
            control={
              <Dialog>
                <DialogTrigger render={<Button variant="outline" />}>
                  Open example dialog
                </DialogTrigger>
                <DialogPopup>
                  <DialogHeader>
                    <DialogTitle>Example dialog</DialogTitle>
                    <DialogDescription>
                      A focused confirmation with a clear title and explicit actions. This example
                      changes no vault data.
                    </DialogDescription>
                  </DialogHeader>
                  <DialogFooter>
                    <DialogClose render={<Button variant="outline" />}>Cancel</DialogClose>
                    <DialogClose render={<Button />}>Done</DialogClose>
                  </DialogFooter>
                </DialogPopup>
              </Dialog>
            }
          />
        </SettingsSection>
      </PageContainer>
    </>
  );
}
