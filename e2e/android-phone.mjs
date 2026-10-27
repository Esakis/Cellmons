import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { readFile, writeFile } from 'node:fs/promises';

// Select the actual game WebView; automatic discovery can wait forever on Android's empty target.
const directory = 'target/phone-test';
const serial = process.env.ANDROID_SERIAL ?? 'emulator-5580';
assert(serial.startsWith('emulator-'), 'Use a task-owned emulator, never a physical phone');
const config = JSON.parse(await readFile(`${directory}/connection.json`, 'utf8'));
assert.equal(config.packageId, 'dev.game.training.phone');
const account = JSON.parse(await readFile(`${directory}/konto-testowe.json`, 'utf8'));
const adb = (...args) =>
  execFileSync(
    `${process.env.LOCALAPPDATA}/Android/Sdk/platform-tools/adb.exe`,
    ['-s', serial, ...args],
    { encoding: 'utf8', timeout: 10000 },
  ).trim();
const pid = adb('shell', 'pidof', config.packageId);
assert(/^\d+$/.test(pid), 'Start the test APK first');
const port = adb('forward', 'tcp:0', `localabstract:webview_devtools_remote_${pid}`);
assert(/^\d+$/.test(port));
const report = { package: config.packageId, apiOrigin: config.apiOrigin, checks: {} };
let socket;
let sequence = 0;
const pending = new Map();
function call(method, params = {}) {
  return new Promise((resolve, reject) => {
    const id = ++sequence;
    const timeout = setTimeout(() => {
      pending.delete(id);
      reject(new Error(`Android debugging timeout: ${method}`));
    }, 15000);
    pending.set(id, { resolve, reject, timeout });
    socket.send(JSON.stringify({ id, method, params }));
  });
}
async function evaluate(expression) {
  const response = await call('Runtime.evaluate', {
    expression,
    returnByValue: true,
    awaitPromise: true,
  });
  if (response.exceptionDetails) {
    const description = response.exceptionDetails.exception?.description?.split('\n')[0];
    throw new Error(
      `Android page evaluation failed: ${description ?? response.exceptionDetails.text}`,
    );
  }
  return response.result.value;
}
async function wait(expression, description) {
  const deadline = Date.now() + 20000;
  while (Date.now() < deadline) {
    if (await evaluate(expression)) return;
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  throw new Error(`Android UI timeout: ${description}`);
}
async function screenshot(name, selector) {
  if (selector)
    await evaluate(
      `document.querySelector(${JSON.stringify(selector)})?.scrollIntoView({block:'center'})`,
    );
  const shot = await call('Page.captureScreenshot', {
    format: 'png',
    captureBeyondViewport: false,
  });
  await writeFile(`${directory}/${name}.png`, Buffer.from(shot.data, 'base64'));
}
const button = (text, root = 'document') =>
  `Array.from(${root}.querySelectorAll('button')).find(e=>{
    const copy=e.cloneNode(true);
    copy.querySelectorAll('[aria-hidden=true]').forEach(node=>node.remove());
    return (e.getAttribute('aria-label')||copy.textContent).trim()===${JSON.stringify(text)};
  })`;

try {
  let target;
  const discoveryDeadline = Date.now() + 20000;
  while (!target && Date.now() < discoveryDeadline) {
    const targets = await (
      await fetch(`http://127.0.0.1:${port}/json/list`, {
        signal: AbortSignal.timeout(5000),
      })
    ).json();
    target = targets.find(
      (entry) => entry.type === 'page' && entry.url.startsWith('https://localhost/'),
    );
    if (!target) await new Promise((resolve) => setTimeout(resolve, 200));
  }
  assert(target, 'Game WebView missing');
  socket = new WebSocket(target.webSocketDebuggerUrl);
  socket.addEventListener('message', (event) => {
    const response = JSON.parse(event.data);
    const request = pending.get(response.id);
    if (!request) return;
    pending.delete(response.id);
    clearTimeout(request.timeout);
    if (response.error) request.reject(new Error(response.error.message));
    else request.resolve(response.result);
  });
  await new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve, { once: true });
    socket.addEventListener('error', reject, { once: true });
  });
  await wait(
    "document.readyState==='complete' && Boolean(document.querySelector('header nav'))",
    'application startup',
  );
  await evaluate(
    "localStorage.setItem('game.language','en'); localStorage.setItem('game.speed','instant'); location.hash='/account'",
  );
  await call('Page.reload');
  await wait(
    `document.querySelector('input[type=password]') || ${button('Sign out')}`,
    'account screen',
  );
  if (await evaluate(`Boolean(${button('Sign out')})`))
    await evaluate(`${button('Sign out')}.click()`);
  await wait("Boolean(document.querySelector('.account-form input[name=identity]'))", 'login form');
  await evaluate(`(() => {
    const form = document.querySelector('.account-form');
    const password = form.querySelector('input[name=password]');
    const identity = form.querySelector('input[name=identity]');
    for (const [element, value] of [[identity, ${JSON.stringify(account.login)}], [password, ${JSON.stringify(account.password)}]]) {
      element.value = value;
      element.dispatchEvent(new Event('input', {bubbles:true}));
      element.dispatchEvent(new Event('change', {bubbles:true}));
    }
    form.requestSubmit();
  })()`);
  await wait(
    `Array.from(document.querySelectorAll('h1,h2')).some(e=>e.textContent.trim()===${JSON.stringify(account.login)})`,
    'native login',
  );
  report.checks.native_login_over_lan_tls = true;
  console.log('Native login passed.');
  report.checks.keystore_session = await evaluate(
    "(async()=>{const s=await window.Capacitor.nativePromise('GamePlatform','loadSession',{});return typeof s.token==='string'&&s.token.length===64})()",
  );
  assert(report.checks.keystore_session);
  await evaluate("location.hash='/collection'");
  // The phone account follows the normal rules: it registers as a Mage and earns further
  // classes through account levels or contracts, so only the starter class is guaranteed.
  await wait("document.querySelectorAll('.collection-avatars button').length>=1", 'own classes');
  const roster = await evaluate(
    "Array.from(document.querySelectorAll('.collection-avatars button span')).map(e=>e.firstChild.textContent.trim())",
  );
  assert(roster.includes('Mage'), `starter class missing: ${roster.join(', ')}`);
  report.roster = roster;
  await evaluate(
    "Array.from(document.querySelectorAll('.collection-avatars button')).find(e=>e.textContent.includes('Mage')).click()",
  );
  await wait("document.querySelector('.collection-heading h2')?.textContent==='Mage'", 'Mage selected');
  if (await evaluate(`Boolean(${button('Activate')})`)) {
    await evaluate(`${button('Activate')}.click()`);
    await wait(`Boolean(${button('Active')})`, 'Mage activated');
  }
  await wait(
    "Array.from(document.querySelectorAll('h2,h3')).some(e=>e.textContent==='Deck (20/20)')",
    'Mage starter deck',
  );
  report.checks.starter_class = true;
  report.checks.starter_deck = true;
  await screenshot('android-starter', '.collection-heading');
  await evaluate("location.hash='/lobby'");
  await wait(
    "document.querySelector('[data-testid=connection-status]')?.textContent==='Connected'",
    'WebSocket connection',
  );
  await wait(`Boolean(${button('Arena')})`, 'arena navigation');
  await evaluate(`${button('Arena')}.click()`);
  await wait(
    "document.querySelectorAll('.lobby-player-list button').length>20",
    'managed accounts',
  );
  report.arenaPopulation = await evaluate(
    "document.querySelectorAll('.lobby-player-list button').length",
  );
  report.checks.persistent_accounts_visible = true;
  await screenshot('android-arena', '.lobby-player-list');
  for (const name of ['LunaVey', 'Mirael', 'Astar', 'Iskra', 'NocnySokol']) {
    const row = `Array.from(document.querySelectorAll('.lobby-player-list button')).find(e=>e.textContent.includes(${JSON.stringify(name)}))`;
    if (!(await evaluate(`Boolean(${row})`))) continue;
    await evaluate(`${row}.click()`);
    await wait(
      `document.querySelector('.lobby-profile h2')?.textContent===${JSON.stringify(name)}`,
      'opponent profile',
    );
    if (!(await evaluate(`${button('Challenge')} && !${button('Challenge')}.disabled`))) continue;
    await evaluate(`${button('Challenge')}.click()`);
    report.opponent = name;
    break;
  }
  assert(report.opponent, 'No idle managed opponent');
  const duel = "document.querySelector('[data-testid=online-duel]')";
  await wait(`Boolean(${duel})`, 'accepted challenge');
  report.checks.challenge_autoaccepted = true;
  await screenshot('android-duel', '[data-testid=online-duel]');
  report.rounds = [];
  for (let round = 0; round < 5; round++) {
    if (await evaluate(`Boolean(${duel}.querySelector('.online-result'))`)) break;
    const heading = await evaluate(`${duel}.querySelector('h1,h2')?.textContent`);
    report.rounds.push(heading);
    await wait(
      `${button('Submit move', duel)} && !${button('Submit move', duel)}.disabled`,
      'move button',
    );
    await evaluate(`${button('Submit move', duel)}.click()`);
    await wait(
      `Boolean(${duel}.querySelector('.online-result')) || ${duel}.querySelector('h1,h2')?.textContent!==${JSON.stringify(heading)}`,
      'server round resolution',
    );
  }
  await wait(`Boolean(${duel}.querySelector('.online-result'))`, 'duel result');
  report.result = await evaluate(`${duel}.querySelector('.online-result').innerText`);
  report.checks.complete_server_duel = true;
  await screenshot('android-result', '.online-result');
  await evaluate(`${button('Close', duel)}.click()`);
  assert(await evaluate('document.documentElement.scrollWidth<=innerWidth'));
  report.checks.no_horizontal_overflow = true;
  console.log(`Android phone checks passed: ${report.opponent}, ${report.rounds.length} rounds.`);
} catch (error) {
  report.failure = error.message;
  if (socket?.readyState === WebSocket.OPEN) await screenshot('android-failure').catch(() => {});
  throw error;
} finally {
  for (const request of pending.values()) clearTimeout(request.timeout);
  if (socket) socket.close();
  await writeFile(`${directory}/android-report.json`, `${JSON.stringify(report, null, 2)}\n`);
  try {
    adb('forward', '--remove', `tcp:${port}`);
  } catch {
    // A failed emulator may already have removed its forwarding socket.
  }
}
