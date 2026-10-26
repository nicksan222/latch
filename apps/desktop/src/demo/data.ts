import type { Entry } from '../_generated/contracts';

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
  return [
    {
      ...base,
      id: `${profileId}-login`,
      name: 'GitHub',
      favorite: true,
      payload: {
        type: 'login',
        data: {
          username: `dev@${profileId}.example`,
          password: null,
          urls: ['https://github.com'],
          totp: null,
          custom: [{ key: 'Organization', value: profileId, hidden: false }]
        }
      },
      redactedFields: ['password']
    },
    {
      ...base,
      id: `${profileId}-database`,
      name: 'Postgres development',
      payload: {
        type: 'database',
        data: {
          engine: 'postgres',
          host: 'localhost',
          port: 5432,
          database: profileId.replaceAll('-', '_'),
          username: 'developer',
          password: null,
          options: 'sslmode=require'
        }
      },
      redactedFields: ['password', 'url']
    },
    {
      ...base,
      id: `${profileId}-api`,
      name: profileId === 'personal' ? 'Cloudflare DNS' : 'Stripe sandbox',
      payload: {
        type: 'api_key',
        data: {
          key: null,
          secret: null,
          endpoint:
            profileId === 'personal'
              ? 'https://api.cloudflare.com/client/v4'
              : 'https://api.stripe.com/v1',
          expiresAt: null
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
        type: 'env',
        data: {
          vars: [
            { key: 'NODE_ENV', value: 'development', hidden: false, comment: null },
            {
              key: 'APP_URL',
              value: 'http://localhost:3000',
              hidden: false,
              comment: 'Local frontend'
            },
            { key: 'DATABASE_URL', value: null, hidden: true, comment: 'Development database' },
            { key: 'STRIPE_SECRET_KEY', value: null, hidden: true, comment: 'Test mode only' },
            { key: 'SESSION_SECRET', value: null, hidden: true, comment: null }
          ]
        }
      },
      redactedFields: ['vars.DATABASE_URL', 'vars.STRIPE_SECRET_KEY', 'vars.SESSION_SECRET']
    },
    {
      ...base,
      id: `${profileId}-deploy`,
      name: 'Deployment token',
      payload: { type: 'secret', data: { value: null } },
      redactedFields: ['value']
    },
    {
      ...base,
      id: `${profileId}-note`,
      name: 'Project runbook',
      payload: { type: 'note', data: { body: null } },
      redactedFields: ['body']
    }
  ];
}

export function demoValue(id: string, field: string): string {
  const entry = demoProfiles
    .flatMap((profile) => demoEntries(profile.id))
    .find((item) => item.id === id);
  if (!entry) throw new Error('Demo entry not found');
  if (field === 'username' && (entry.payload.type === 'login' || entry.payload.type === 'database'))
    return entry.payload.data.username;
  if (!entry.redactedFields.includes(field)) throw new Error('Demo field not found');
  if (field === 'body')
    return (
      'Fictional project runbook\n\n1. Start your local database.\n2. Run latch env ' +
      entry.profileId +
      ' -- npm run dev.\n3. Ship something good.'
    );
  if (field === 'url') return 'postgres://developer:demo-only@localhost:5432/' + entry.profileId;
  if (field === 'vars.DATABASE_URL')
    return 'postgres://developer:demo-only@localhost:5432/' + entry.profileId;
  if (field.startsWith('vars.') || field === 'value')
    return 'demo_only_' + entry.profileId + '_' + field.replace('vars.', '').toLowerCase();
  if (field === 'key') return 'demo_key_not_a_real_credential';
  return 'demo-only-' + entry.profileId + '-password';
}
