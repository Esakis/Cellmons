import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';

const documents = JSON.parse(await readFile('legal/documents.json', 'utf8'));
const site = JSON.parse(
  await readFile(process.env.LEGAL_CONFIG ?? 'legal/site.example.json', 'utf8'),
);
const fields = [
  'operator',
  'postal_address',
  'support_email',
  'privacy_email',
  'security_email',
  'processors',
  'transfers',
  'security_retention',
  'backup_retention',
];
for (const key of fields)
  if (typeof site[key] !== 'string' || site[key].length > 4000)
    throw new Error(`Invalid public legal field: ${key}`);
const approved =
  site.approved_version === documents.version && fields.every((key) => site[key].trim().length > 0);
for (const key of ['support_email', 'privacy_email', 'security_email'])
  if (site[key] && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(site[key]))
    throw new Error(`Invalid public legal email: ${key}`);
if (process.env.PUBLIC_RELEASE === '1' && !approved)
  throw new Error(
    'Public release requires completed LEGAL_CONFIG and owner legal approval for the document version',
  );
const credits = await Promise.all(
  ['OFL.txt', 'OFL-Cinzel.txt'].map(
    async (name) => await readFile(`web/src/styles/fonts/${name}`, 'utf8'),
  ),
);
const payload = { ...documents, site, draft: !approved, credits };
await mkdir('web/src/generated', { recursive: true });
await writeFile('web/src/generated/legal.json', JSON.stringify(payload));
await writeFile(
  'web/src/generated/legal-version.json',
  JSON.stringify({ version: documents.version }),
);
const output = resolve('web/public/legal');
await mkdir(output, { recursive: true });
const escapeHtml = (value) =>
  String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;');
const nav =
  '<nav><a href="index.html">PL / EN</a> · <a href="terms.html">Regulamin / Terms</a> · <a href="privacy.html">Prywatność / Privacy</a> · <a href="deletion.html">Usuń konto / Delete account</a> · <a href="credits.html">Licencje / Licenses</a></nav>';
for (const kind of ['index', 'terms', 'privacy', 'deletion', 'credits']) {
  let body = nav;
  if (kind === 'index')
    body +=
      '<h1>Spellgambit: Arcane Class</h1><p>Regulamin, prywatność i usunięcie konta / Terms, privacy and account deletion.</p>';
  if (kind === 'credits')
    body +=
      '<h1>Licencje / Licenses</h1><p><a href="software.txt">Licencje oprogramowania / Software licenses</a></p>' +
      credits.map((text) => `<pre lang="en">${escapeHtml(text)}</pre>`).join('');
  if (['terms', 'privacy', 'deletion'].includes(kind))
    for (const lang of ['pl', 'en']) {
      const doc = documents[lang][kind];
      body += `<article lang="${lang}"><h1>${escapeHtml(doc.title)}</h1><p>${escapeHtml(documents.version)}</p>${!approved ? `<p role="status">${escapeHtml(documents[lang].draft)}</p>` : ''}`;
      body += doc.sections
        .map(
          ([title, text]) =>
            `<section><h2>${escapeHtml(title)}</h2><p>${escapeHtml(text)}</p></section>`,
        )
        .join('');
      if (kind === 'deletion')
        body +=
          '<p><a href="../#/account?privacy=1">Otwórz panel danych / Open the data panel</a></p>';
      body += `<h2>${escapeHtml(documents[lang].contact)}</h2><dl>${fields.map((key) => `<dt>${escapeHtml({ operator: 'Operator / administrator', postal_address: 'Adres / address', support_email: 'Pomoc / support', privacy_email: 'Prywatność / privacy', security_email: 'Bezpieczeństwo / security', processors: 'Podmioty przetwarzające / processors', transfers: 'Transfery / transfers', security_retention: 'Retencja bezpieczeństwa / security retention', backup_retention: 'Retencja kopii / backup retention' }[key])}</dt><dd>${escapeHtml(site[key] || '—')}</dd>`).join('')}</dl></article>`;
    }
  await writeFile(
    `${output}/${kind}.html`,
    `<!doctype html><html lang="pl"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Spellgambit: Arcane Class — ${escapeHtml(kind)}</title><link rel="stylesheet" href="style.css"><body><main>${body}</main></body></html>`,
  );
}
await writeFile(
  `${output}/style.css`,
  'body{font:18px/1.65 system-ui,sans-serif;max-width:80ch;margin:auto;padding:24px;color:#18131d;background:#fff}a{color:#493177}nav{display:flex;flex-wrap:wrap;gap:12px}article{margin-top:48px}pre{white-space:pre-wrap;overflow-wrap:anywhere}dd{margin:0 0 16px;white-space:pre-wrap;overflow-wrap:anywhere}dt{font-weight:bold}:focus-visible{outline:3px solid #493177;outline-offset:4px}',
);
await writeFile(
  'web/public/legal-assets.json',
  JSON.stringify(
    ['index', 'terms', 'privacy', 'deletion', 'credits']
      .map((kind) => `legal/${kind}.html`)
      .concat('legal/style.css', 'legal/software.txt', 'legal-assets.json'),
  ),
);
