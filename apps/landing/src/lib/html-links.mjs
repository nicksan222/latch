/** URL-bearing document elements, excluding React's <style href> resource keys. */
export function documentLinks(html) {
  const links = [];
  for (const [element] of html.matchAll(/<(?:a|link|script|img|source|iframe|video|audio|use)\b[^>]*>/gi)) {
    for (const [, attribute, url] of element.matchAll(/\s(href|src)="([^"]+)"/g)) links.push([attribute, url]);
  }
  return links;
}
