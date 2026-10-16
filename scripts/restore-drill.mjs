// Encrypted pgBackRest PITR on two NEW local volumes. No application .env or existing DB.
import { spawn } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { setTimeout as sleep } from 'node:timers/promises';

const id = `game-pitr-${randomBytes(6).toString('hex')}`;
const directory = resolve('target', id);
await mkdir(directory, { recursive: true });
const image = process.env.PITR_IMAGE ?? 'spellgambit-postgres:local';
const primary = `${id}-primary`,
  restored = `${id}-restored`;
const volumes = [`${id}-data`, `${id}-repo`, `${id}-restore`];
const config = resolve(directory, 'pgbackrest.conf');
await writeFile(
  config,
  `[global]\nrepo1-path=/var/lib/pgbackrest\nrepo1-cipher-type=aes-256-cbc\nrepo1-cipher-pass=${randomBytes(32).toString('hex')}\nstart-fast=y\nlog-level-console=warn\nlog-level-file=off\n[game]\npg1-path=/var/lib/postgresql/18/docker\n`,
  { mode: 0o600 },
);
async function run(command, args, input, env) {
  return new Promise((resolveRun, reject) => {
    const child = spawn(command, args, {
      env: env ?? process.env,
      windowsHide: true,
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    let stdout = '',
      stderr = '';
    const timer = setTimeout(() => child.kill(), 240_000);
    child.stdout.on('data', (chunk) => {
      stdout += chunk;
    });
    child.stderr.on('data', (chunk) => {
      stderr += chunk;
    });
    child.on('error', reject);
    child.on('close', (code) => {
      clearTimeout(timer);
      if (code === 0) resolveRun(stdout.trim());
      else
        reject(
          new Error(`${command} failed (${code}): ${stderr.slice(-2500)}\n${stdout.slice(-2500)}`),
        );
    });
    child.stdin.end(input);
  });
}
const docker = (...args) => run('docker', args);
const mounts = [
  '-v',
  `${volumes[1]}:/var/lib/pgbackrest`,
  '-v',
  `${config}:/etc/pgbackrest/pgbackrest.conf:ro`,
];
const sql = (container, query) =>
  run(
    'docker',
    [
      'exec',
      '-i',
      container,
      'psql',
      '-U',
      'postgres',
      '-d',
      'game',
      '-v',
      'ON_ERROR_STOP=1',
      '-qAt',
    ],
    query,
  );
async function ready(container) {
  for (let i = 0; i < 90; i++) {
    if (
      await docker('exec', container, 'pg_isready', '-U', 'postgres', '-d', 'game').then(
        () => true,
        () => false,
      )
    )
      return;
    await sleep(1000);
  }
  throw new Error('Fresh restore did not become ready');
}
async function binary(container, command) {
  if (process.env.PITR_SERVER_IMAGE)
    return docker(
      'run',
      '--rm',
      '--network',
      `container:${container}`,
      '--read-only',
      '--cap-drop',
      'ALL',
      '--security-opt',
      'no-new-privileges',
      '-e',
      'LISTEN_ADDR=127.0.0.1:8080',
      '-e',
      'PUBLIC_ORIGIN=http://localhost:5173',
      '-e',
      'DATABASE_URL=postgres://postgres:fixture@127.0.0.1:5432/game',
      process.env.PITR_SERVER_IMAGE,
      command,
    );
  const port = await docker('port', container, '5432');
  if (!/^127\.0\.0\.1:\d+$/.test(port)) throw new Error('Unexpected fixture port');
  return run(
    resolve('target/debug', process.platform === 'win32' ? 'server.exe' : 'server'),
    [command],
    undefined,
    {
      ...process.env,
      DATABASE_URL: `postgres://postgres:fixture@${port}/game`,
      MIGRATION_DATABASE_URL: `postgres://postgres:fixture@${port}/game`,
    },
  );
}
async function runtimeSmoke(container) {
  if (!process.env.PITR_SERVER_IMAGE) return;
  const app = `${id}-app`;
  try {
    await docker(
      'run',
      '-d',
      '--name',
      app,
      '--network',
      `container:${container}`,
      '--read-only',
      '--cap-drop',
      'ALL',
      '--security-opt',
      'no-new-privileges',
      '-e',
      'LISTEN_ADDR=127.0.0.1:8080',
      '-e',
      'OPS_LISTEN_ADDR=127.0.0.1:9090',
      '-e',
      'PUBLIC_ORIGIN=http://localhost:5173',
      '-e',
      'DATABASE_URL=postgres://postgres:fixture@127.0.0.1:5432/game',
      process.env.PITR_SERVER_IMAGE,
    );
    await docker(
      'run',
      '--rm',
      '--network',
      `container:${container}`,
      'node:24.19.0-bookworm-slim',
      'node',
      '-e',
      `const assert=require('node:assert/strict');(async()=>{let ready=false;for(let i=0;i<60;i++){try{const r=await fetch('http://127.0.0.1:8080/readyz');if(r.ok){ready=true;break}}catch{}await new Promise(r=>setTimeout(r,500))}assert(ready);assert((await fetch('http://127.0.0.1:8080/healthz')).ok);assert.equal((await fetch('http://127.0.0.1:8080/metrics')).status,404);const metrics=await fetch('http://127.0.0.1:9090/metrics');assert(metrics.ok);assert((await metrics.text()).includes('wal_archive_failed 0'));console.log('Static Linux runtime: health, readiness, private metrics and PostgreSQL passed')})().catch(e=>{console.error(e);process.exit(1)})`,
    );
  } finally {
    await docker('rm', '-f', app).catch(() => {});
  }
}
const started = performance.now();
try {
  for (const volume of volumes) await docker('volume', 'create', volume);
  for (const volume of [volumes[0], volumes[2]])
    await docker(
      'run',
      '--rm',
      '-u',
      '0',
      '-v',
      `${volume}:/data`,
      image,
      'sh',
      '-c',
      'mkdir -p /data/18/docker && chown -R postgres:postgres /data',
    );
  await docker(
    'run',
    '--rm',
    '-u',
    '0',
    ...mounts,
    image,
    'chown',
    'postgres:postgres',
    '/var/lib/pgbackrest',
  );
  await docker(
    'run',
    '-d',
    '--name',
    primary,
    '-e',
    'POSTGRES_PASSWORD=fixture',
    '-e',
    'POSTGRES_DB=game',
    '-p',
    '127.0.0.1::5432',
    '-v',
    `${volumes[0]}:/var/lib/postgresql`,
    ...mounts,
    image,
    'postgres',
    '-c',
    'archive_mode=on',
    '-c',
    'archive_command=pgbackrest --stanza=game archive-push %p',
  );
  await ready(primary);
  await binary(primary, 'migrate');
  if (process.env.PITR_SERVER_IMAGE) {
    await binary(primary, 'check-config');
  }
  await sql(
    primary,
    `BEGIN;
INSERT INTO users(id,login,email,password_hash,start_school) VALUES ('00000000-0000-0000-0000-000000000001','PitrFixture','pitr@example.test','$argon2id$v=19$m=19456,t=2,p=1$fixture$fixture','combat');
INSERT INTO wallets(user_id,balance) VALUES ('00000000-0000-0000-0000-000000000001',40);
INSERT INTO ledger_transactions(id,reason) VALUES ('00000000-0000-0000-0000-000000000002','start');
INSERT INTO ledger_entries(tx_id,account,delta) VALUES ('00000000-0000-0000-0000-000000000002','user:00000000-0000-0000-0000-000000000001',40),('00000000-0000-0000-0000-000000000002','system:faucet',-40);
CREATE TABLE restore_probe(k integer PRIMARY KEY); COMMIT;`,
  );
  const backrest = (...args) =>
    docker('exec', '-u', 'postgres', primary, 'pgbackrest', '--stanza=game', ...args);
  await backrest('stanza-create');
  await backrest('check');
  await backrest('--type=full', 'backup');
  await runtimeSmoke(primary);
  // pgBackRest records the backup stop with whole-second precision. A target
  // in the same second can sort before that stop and select no usable backup.
  await sleep(1500);
  await sql(primary, 'INSERT INTO restore_probe VALUES (1);');
  const target = await sql(primary, 'SELECT clock_timestamp();');
  await sleep(1100);
  await sql(primary, 'INSERT INTO restore_probe VALUES (2); SELECT pg_switch_wal();');
  await backrest('check');
  await docker('stop', '--time', '15', primary);
  const restoreStarted = performance.now();
  await docker(
    'run',
    '--rm',
    '-u',
    'postgres',
    '-v',
    `${volumes[2]}:/var/lib/postgresql`,
    ...mounts,
    image,
    'pgbackrest',
    '--stanza=game',
    '--type=time',
    `--target=${target}`,
    '--target-action=promote',
    'restore',
  );
  await docker(
    'run',
    '-d',
    '--name',
    restored,
    '-p',
    '127.0.0.1::5432',
    '-v',
    `${volumes[2]}:/var/lib/postgresql`,
    ...mounts,
    image,
    'postgres',
    '-c',
    'archive_mode=off',
  );
  await ready(restored);
  const markers = await sql(
    restored,
    "SELECT string_agg(k::text,',' ORDER BY k) FROM restore_probe;",
  );
  if (markers !== '1') throw new Error('PITR boundary failed: expected before marker only');
  const ledger = JSON.parse(
    (await binary(restored, 'reconcile')).split('\n').find((line) => line.trim().startsWith('{')),
  );
  if (Object.values(ledger).some((value) => value !== 0))
    throw new Error('Restored ledger does not reconcile');
  const checks = JSON.parse(
    await sql(restored, await readFile('deploy/postgres/restore-check.sql', 'utf8')),
  );
  if (
    checks.users !== 1 ||
    checks.ledger_entries !== 2 ||
    checks.incorrect_wallets !== 0 ||
    checks.unbalanced_transactions !== 0 ||
    checks.incorrect_escrows !== 0
  )
    throw new Error('Production restore verification query failed');
  const report = {
    schema: 1,
    success: true,
    synthetic: true,
    repository: 'encrypted local volume (S3 acceptance remains external)',
    target,
    restore_ms: Math.round(performance.now() - restoreStarted),
    total_ms: Math.round(performance.now() - started),
    markers,
    ledger,
    checks,
    runtime_image_tested: process.env.PITR_SERVER_IMAGE ?? null,
  };
  await writeFile('target/f14-restore-drill.json', `${JSON.stringify(report, null, 2)}\n`);
  console.log(JSON.stringify(report));
} finally {
  for (const container of [primary, restored]) await docker('rm', '-f', container).catch(() => {});
  for (const volume of volumes) await docker('volume', 'rm', volume).catch(() => {});
}
