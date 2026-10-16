import { readFile, writeFile } from 'node:fs/promises';

// OSV's public Maven advisory database. Only public package coordinates leave the machine.
const dependencies = JSON.parse(await readFile('target/android-dependencies.json', 'utf8'));
if (!Array.isArray(dependencies) || dependencies.length === 0 || dependencies.length > 1000)
  throw new Error('Run the Android dependencyInventory task first');
const queries = dependencies.map((coordinate) => {
  if (!/^[\w.-]+:[\w.-]+:[\w.+-]+$/.test(coordinate)) throw new Error('Invalid Maven coordinate');
  const [group, artifact, version] = coordinate.split(':');
  return { package: { ecosystem: 'Maven', name: `${group}:${artifact}` }, version };
});
const response = await fetch('https://api.osv.dev/v1/querybatch', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ queries }),
  signal: AbortSignal.timeout(60000),
});
if (!response.ok) throw new Error(`OSV unavailable (${response.status})`);
const { results } = await response.json();
if (!Array.isArray(results) || results.length !== dependencies.length)
  throw new Error('Incomplete OSV response');
const findings = results.flatMap((result, index) =>
  (result.vulns ?? []).map(({ id }) => ({ dependency: dependencies[index], id })),
);
await writeFile(
  'target/android-advisories.json',
  `${JSON.stringify({ checked_at: new Date().toISOString(), components: dependencies.length, findings }, null, 2)}\n`,
);
console.log(
  `Android Maven dependencies: ${dependencies.length}; advisory matches: ${findings.length}`,
);
// No blanket exceptions: unresolved advisory matches fail the gate for review.
if (findings.length) process.exitCode = 1;
