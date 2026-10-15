import { access } from 'node:fs/promises';
export default async function teardown() {
  await fetch('http://127.0.0.1:18092/__shutdown', {
    method: 'POST',
    signal: AbortSignal.timeout(3000),
  });
  const deadline = Date.now() + 10000;
  while (Date.now() < deadline) {
    try {
      await access(new URL('../target/e2e-backend-running', import.meta.url));
    } catch {
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error('Playwright backend did not stop');
}
