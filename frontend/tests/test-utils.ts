/** Wrap a mock fetch implementation — satisfies strict `typeof fetch` typing. */
export function makeFetch(
  fn: (url: string, init?: RequestInit) => Promise<Response>,
): typeof fetch {
  return fn as unknown as typeof fetch;
}
