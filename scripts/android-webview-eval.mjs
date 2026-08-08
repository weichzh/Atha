// Description: Evaluates a bounded expression in an Android reader WebView over CDP.

const [port, expression] = process.argv.slice(2);
if (!/^\d+$/.test(port ?? '') || !expression) process.exit(2);

const targets = await fetch(`http://127.0.0.1:${port}/json`, {
  signal: AbortSignal.timeout(5000),
}).then((response) => response.json());
const target = targets.find(
  (entry) => entry.type === 'page' && entry.url?.startsWith('https://tauri.localhost/'),
);
if (!target?.webSocketDebuggerUrl) process.exit(3);

const socket = new WebSocket(target.webSocketDebuggerUrl);
const timeout = setTimeout(() => {
  socket.close();
  process.exit(4);
}, 20000);

socket.addEventListener('open', () => {
  socket.send(JSON.stringify({
    id: 1,
    method: 'Runtime.evaluate',
    params: { expression, awaitPromise: true, returnByValue: true },
  }));
});
socket.addEventListener('message', ({ data }) => {
  const message = JSON.parse(data);
  if (message.id !== 1) return;
  clearTimeout(timeout);
  socket.close();
  if (message.error || message.result?.exceptionDetails) process.exit(5);
  process.stdout.write(JSON.stringify(message.result?.result?.value ?? null));
});
socket.addEventListener('error', () => {
  clearTimeout(timeout);
  process.exit(6);
});
