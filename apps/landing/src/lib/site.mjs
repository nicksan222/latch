/** Resolve local, GitHub project/user Pages, and custom-domain deployments. */
export function resolveSite(env = process.env) {
  const slug = env.GITHUB_REPOSITORY;
  const repository = env.LANDING_REPOSITORY || `https://github.com/${slug || 'nicksan222/latch'}`;
  const [owner, repo] = (slug || '').split('/');
  const inferred = owner ? `https://${owner}.github.io/${repo?.toLowerCase() === `${owner.toLowerCase()}.github.io` ? '' : `${repo}/`}` : 'http://localhost:4321/';
  const url = new URL(env.LANDING_SITE || inferred);
  return { site: url.origin, base: env.LANDING_BASE || url.pathname.replace(/\/$/, '') || '/', repository };
}
export function withBase(base, path = '') {
  return `${base.replace(/\/$/, '')}/${path.replace(/^\//, '')}`;
}
