import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';

// Explicit local fixture setup. Run before Android login; it rotates the test account's session.
// Only the generated Telefon_* account in the local Docker database can receive these unlocks.
const account = JSON.parse(await readFile('target/phone-test/konto-testowe.json', 'utf8'));
assert(/^Telefon_[0-9a-f]{6}$/.test(account.login));
assert.equal(account.email, `${account.login.toLowerCase()}@phone-test.invalid`);
const ids = [1, 2, 3, 4, 5, 11, 13, 32];
const headers = {
  'Content-Type': 'application/json',
  'X-Requested-With': 'app',
  'X-Native-Version': '1.0.1',
  Origin: 'https://localhost',
};
async function api(path, body) {
  const response = await fetch(`http://127.0.0.1:8090/api/${path}`, {
    signal: AbortSignal.timeout(10000),
    method: body ? 'POST' : 'GET',
    headers,
    body: body ? JSON.stringify(body) : undefined,
  });
  const data = response.status === 204 ? null : await response.json();
  assert(response.ok, `${path}: ${response.status}, ${data?.error}`);
  return data;
}
const session = await api('auth/login', {
  identity: account.login,
  password: account.password,
  client: 'android',
});
headers.Authorization = `Bearer ${session.token}`;
execFileSync(
  'docker',
  [
    'compose',
    '-f',
    'deploy/docker-compose.dev.yml',
    'exec',
    '-T',
    'postgres',
    'psql',
    '-U',
    'game',
    '-d',
    'game',
    '-v',
    'ON_ERROR_STOP=1',
    '-c',
    `INSERT INTO unlocked_templates(user_id,template_id,source)
   SELECT id,t,'admin' FROM users CROSS JOIN unnest(ARRAY[${ids.join(',')}]) t
   WHERE login='${account.login}' AND email='${account.email}'
   ON CONFLICT DO NOTHING`,
  ],
  { encoding: 'utf8', timeout: 15000 },
);
let snapshot = await api('game');
for (const id of ids) {
  if (snapshot.avatars.some((a) => a.template_id === id)) continue;
  snapshot = await api('game', { action: 'create', id: randomUUID(), template_id: id });
}
assert.deepEqual(
  [...snapshot.templates].sort((a, b) => a - b),
  ids,
);
for (const id of ids) {
  assert.equal(snapshot.avatars.find((a) => a.template_id === id).deck.length, 20);
}
const vampire = snapshot.avatars.find((a) => a.template_id === 32);
snapshot = await api('game', { action: 'activate', id: vampire.id });
await writeFile(
  'target/phone-test/roster-report.json',
  `${JSON.stringify(
    {
      login: account.login,
      templates: ids,
      avatars: snapshot.avatars.map((a) => ({
        template: a.template_id,
        level: a.level,
        active: a.active,
        cards: a.deck.length,
      })),
    },
    null,
    2,
  )}\n`,
);
await api('auth/logout', {});
console.log(
  'Dedicated phone account ready: eight classes, eight legal 20-card decks, Vampire active.',
);
