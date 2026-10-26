import { create, type MessageInitShape } from '@bufbuild/protobuf';
import { EntrySchema, type Entry } from '@latch/rpc/v1';

export const demoProfiles = [
  {
    id: 'acme',
    name: 'acme',
    description: 'Acme Studio · client applications',
    icon: 'briefcase',
    color: '#3568e8'
  },
  {
    id: 'personal',
    name: 'personal',
    description: 'Personal accounts & infrastructure',
    icon: 'user',
    color: '#8b5cf6'
  },
  {
    id: 'side-project',
    name: 'side-project',
    description: 'Launchpad · weekend SaaS',
    icon: 'rocket',
    color: '#16a34a'
  }
];

/** Deliberately fictional, public demo data. Never connected to a vault. */
export function demoEntries(profileId: string): Entry[] {
  const base = {
    profileId,
    tags: [profileId === 'acme' ? 'work' : profileId === 'personal' ? 'personal' : 'launchpad'],
    notes: '',
    favorite: false,
    createdAt: '2026-10-25T12:00:00Z',
    updatedAt: '2026-10-25T12:00:00Z'
  };
  const entries: MessageInitShape<typeof EntrySchema>[] = [
    {
      ...base,
      id: `${profileId}-login`,
      name: 'GitHub',
      favorite: true,
      payload: {
        data: {
          case: 'login',
          value: {
            username: `dev@${profileId}.example`,
            password: undefined,
            urls: ['https://github.com'],
            totp: undefined,
            custom: [{ key: 'Organization', value: profileId, hidden: false }]
          }
        }
      },
      redactedFields: ['password']
    },
    {
      ...base,
      id: `${profileId}-database`,
      name: 'Postgres development',
      payload: {
        data: {
          case: 'database',
          value: {
            engine: 'postgres',
            host: 'localhost',
            port: 5432,
            database: profileId.replaceAll('-', '_'),
            username: 'developer',
            password: undefined,
            options: 'sslmode=require'
          }
        }
      },
      redactedFields: ['password', 'url']
    },
    {
      ...base,
      id: `${profileId}-api`,
      name: profileId === 'personal' ? 'Cloudflare DNS' : 'Stripe sandbox',
      payload: {
        data: {
          case: 'apiKey',
          value: {
            key: undefined,
            secret: undefined,
            endpoint:
              profileId === 'personal'
                ? 'https://api.cloudflare.com/client/v4'
                : 'https://api.stripe.com/v1',
            expiresAt: undefined
          }
        }
      },
      redactedFields: ['key']
    },
    {
      ...base,
      id: `${profileId}-env`,
      name: 'Development .env',
      favorite: true,
      payload: {
        data: {
          case: 'env',
          value: {
            vars: [
              { key: 'NODE_ENV', value: 'development', hidden: false, comment: undefined },
              {
                key: 'APP_URL',
                value: 'http://localhost:3000',
                hidden: false,
                comment: 'Local frontend'
              },
              {
                key: 'DATABASE_URL',
                value: undefined,
                hidden: true,
                comment: 'Development database'
              },
              {
                key: 'STRIPE_SECRET_KEY',
                value: undefined,
                hidden: true,
                comment: 'Test mode only'
              },
              { key: 'SESSION_SECRET', value: undefined, hidden: true, comment: undefined }
            ]
          }
        }
      },
      redactedFields: ['vars.DATABASE_URL', 'vars.STRIPE_SECRET_KEY', 'vars.SESSION_SECRET']
    },
    {
      ...base,
      id: `${profileId}-deploy`,
      name: 'Deployment token',
      payload: { data: { case: 'secret', value: { value: undefined } } },
      redactedFields: ['value']
    },
    {
      ...base,
      id: `${profileId}-note`,
      name: 'Project runbook',
      payload: { data: { case: 'note', value: { body: undefined } } },
      redactedFields: ['body']
    }
  ];
  return entries.map((entry) => create(EntrySchema, entry));
}

export function demoValue(id: string, field: string): string {
  const entry = demoProfiles
    .flatMap((profile) => demoEntries(profile.id))
    .find((item) => item.id === id);
  if (!entry) throw new Error('Demo entry not found');
  if (
    field === 'username' &&
    (entry.payload?.data.case === 'login' || entry.payload?.data.case === 'database')
  )
    return entry.payload.data.value.username;
  if (!entry.redactedFields.includes(field)) throw new Error('Demo field not found');
  if (field === 'body')
    return (
      'Fictional project runbook\n\n1. Start your local database.\n2. Run latch env ' +
      entry.profileId +
      ' -- bun run dev.\n3. Ship something good.'
    );
  if (field === 'url') return 'postgres://developer:demo-only@localhost:5432/' + entry.profileId;
  if (field === 'vars.DATABASE_URL')
    return 'postgres://developer:demo-only@localhost:5432/' + entry.profileId;
  if (field.startsWith('vars.') || field === 'value')
    return 'demo_only_' + entry.profileId + '_' + field.replace('vars.', '').toLowerCase();
  if (field === 'key') return 'demo_key_not_a_real_credential';
  return 'demo-only-' + entry.profileId + '-password';
}
