// Run the existing standalone reader diagnostics in this candidate's WebKit.
// Only the two checked-in stylesheets are supplied by the test runner; all book
// requests still go through the candidate's active atha-book protocol and CSP.
const [html, theme, readerStyle] = arguments;
const parsed = new DOMParser().parseFromString(html, "text/html");
parsed.querySelectorAll("link, script").forEach((element) => element.remove());
const url = new URL(location.href);
url.searchParams.set("verify-import", "1");
url.searchParams.set("gesture-probe", "1");
url.searchParams.set("probe", "http://127.0.0.1:9/blocked.png");
history.replaceState(null, "", url);
document.open();
document.write("<!doctype html>" + parsed.documentElement.outerHTML);
document.close();
delete window.athaMessages; // The standalone template has no native conversation UI.
for (const selector of ["#update-selection", "#note-selection"]) {
  const button = document.querySelector(selector);
  const label = document.createElement("span");
  label.textContent = button.textContent;
  button.replaceChildren(label);
}
// Standalone kernels emit pipe-delimited host telemetry. Record it locally,
// rather than sending it to the production Tauri JSON-RPC channel.
window.__athaKernelEmissions = [];
window.athaReaderIpc = { postMessage(message) { window.__athaKernelEmissions.push(message); } };

const styles = new Map();
for (const [id, css] of [["reader-theme-source", theme], ["reader-style-source", readerStyle]]) {
  const node = document.createElement("style");
  node.id = id;
  node.textContent = css;
  node.href = new URL(`/linux-kernel-fixture/${id}.css`, location.href).href;
  styles.set(node.href, css);
  document.head.append(node);
}
const nativeFetch = globalThis.fetch.bind(globalThis);
globalThis.fetch = (input, init) =>
  styles.has(input) ? Promise.resolve(new Response(styles.get(input))) : nativeFetch(input, init);
