import defaultSettings from '@/settings'

export default function getPageTitle(pageTitle?: string): string {
  const title = (defaultSettings.title as string) || 'GBServer'
  if (pageTitle) return `${pageTitle} - ${title}`
  return title
}
