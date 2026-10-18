/** Vite uses /dev-api; the backend's same-origin static deployment uses /api. */
export function apiPath(path: string): string {
  const prefix = process.env.E2E_API_PREFIX ?? '/dev-api';
  if (prefix !== '/api' && prefix !== '/dev-api') {
    throw new Error('E2E_API_PREFIX must be /api or /dev-api');
  }
  return `${prefix}${path.startsWith('/') ? path : `/${path}`}`;
}
