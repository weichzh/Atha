export function isReaderRoute(search = location.search): boolean {
  const params = new URLSearchParams(search);
  return params.has("manifest") || params.has("entry") || params.has("book");
}
