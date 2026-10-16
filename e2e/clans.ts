import { expect, type Page } from '@playwright/test';
import { accessible } from './accessibility';

export async function clans(a: Page, b: Page, alice: string, bob: string, project: string) {
  await a.goto('/#/clans');
  await b.goto('/#/clans');
  const guild = `Guild ${alice}`;
  await a.getByLabel('Clan name', { exact: true }).fill(guild);
  await a.getByRole('button', { name: 'Create clan', exact: true }).click();
  const profileA = a.getByRole('region', { name: 'Clan profile', exact: true });
  const profileB = b.getByRole('region', { name: 'Clan profile', exact: true });
  await expect(profileA).toContainText('Members: 1');
  await b.getByLabel('Find a clan', { exact: true }).fill(guild);
  await b.getByRole('button', { name: 'Find a clan', exact: true }).click();
  await b.getByRole('button', { name: guild, exact: true }).click();
  await profileB.getByRole('button', { name: 'Apply to clan', exact: true }).click();
  await expect(b.getByRole('heading', { name: 'Your applications', exact: true })).toBeVisible();
  await a.getByRole('button', { name: 'Refresh', exact: true }).click();
  await profileA.getByRole('button', { name: `Accept: ${bob}`, exact: true }).click();
  await expect(profileA).toContainText('Members: 2');
  await profileA.getByLabel('Public description', { exact: true }).fill('Draft to preserve.');
  let dismissed = false;
  a.once('dialog', async (dialog) => {
    dismissed = true;
    await dialog.dismiss();
  });
  await profileA
    .getByRole('combobox', { name: `Role: ${bob}`, exact: true })
    .selectOption('scribe');
  expect(dismissed).toBe(true);
  await expect(profileA.getByRole('combobox', { name: `Role: ${bob}`, exact: true })).toHaveValue(
    'member',
  );
  await expect(profileA.getByLabel('Public description', { exact: true })).toHaveValue(
    'Draft to preserve.',
  );
  await profileA.getByRole('button', { name: 'Save profile', exact: true }).click();
  await expect(a.getByRole('button', { name: 'Refresh', exact: true })).toBeEnabled();
  await profileA
    .getByRole('combobox', { name: `Role: ${bob}`, exact: true })
    .selectOption('scribe');
  await expect(profileA.getByRole('combobox', { name: `Role: ${bob}`, exact: true })).toHaveValue(
    'scribe',
  );
  await expect(a.getByRole('button', { name: 'Refresh', exact: true })).toBeEnabled();
  await b.getByRole('button', { name: 'Refresh', exact: true }).click();
  await expect(profileB).toContainText('Your role: Scribe');
  await profileB
    .getByLabel('Public description', { exact: true })
    .fill('The clan prepares for tournaments.');
  await profileB.getByLabel('Member notice', { exact: true }).fill('Private meeting at dusk.');
  await expect(profileB.getByLabel('Clan website (HTTPS)', { exact: true })).toBeDisabled();
  await profileB.getByRole('button', { name: 'Save profile', exact: true }).click();
  await expect(b.getByRole('status').filter({ hasText: 'Change saved.' })).toBeVisible();
  await a.getByRole('button', { name: 'Refresh', exact: true }).click();
  await expect(
    profileA.getByText('The clan prepares for tournaments.', { exact: true }),
  ).toBeVisible();
  await accessible(a);
  await a.screenshot({ path: `../target/clans-${project}.png`, fullPage: true });
  expect(
    await a.evaluate(
      () => document.documentElement.scrollWidth > document.documentElement.clientWidth,
    ),
  ).toBe(false);
  await profileA.getByRole('button', { name: `Transfer master to: ${bob}`, exact: true }).click();
  await a.getByRole('dialog').getByRole('button', { name: 'Confirm', exact: true }).click();
  await expect(profileA).toContainText('Your role: Vice-master');
  await b.getByRole('button', { name: 'Refresh', exact: true }).click();
  await expect(profileB).toContainText('Your role: Master');
  await profileB.getByRole('button', { name: `Remove from clan: ${alice}`, exact: true }).click();
  await b.getByRole('dialog').getByRole('button', { name: 'Confirm', exact: true }).click();
  await expect(profileB).toContainText('Members: 1');
  await a.getByRole('button', { name: 'Refresh', exact: true }).click();
  await expect(a.getByRole('heading', { name: 'Create clan', exact: true })).toBeVisible();
  await expect(profileA).not.toContainText('Private meeting at dusk.');
  await expect(profileA.getByRole('heading', { name: 'Edit profile', exact: true })).toHaveCount(0);
  await profileB.getByRole('button', { name: 'Disband clan', exact: true }).click();
  await b.getByRole('dialog').getByRole('button', { name: 'Confirm', exact: true }).click();
  await expect(b.getByRole('heading', { name: 'Create clan', exact: true })).toBeVisible();
  await a.getByRole('button', { name: 'Refresh', exact: true }).click();
  await expect(profileA).toHaveCount(0);
}
