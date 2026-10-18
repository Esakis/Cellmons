import { create } from '@bufbuild/protobuf';
import { cleanup, fireEvent, render, waitFor } from '@solidjs/testing-library';
import { afterEach, expect, it, vi } from 'vitest';
import { RankingSnapshotSchema } from '../../generated/proto/game/v1/online_duel_pb';
import { addTranslations, setLanguage } from '../../i18n';
import type { ClientBody, ServerBody } from '../../net/client';
import en from './en.ftl?raw';
import { OnlineRecords } from './OnlineRecords';
import pl from './pl.ftl?raw';

addTranslations({ pl, en });
afterEach(cleanup);

it('continues ranking numbering on subsequent pages and resets it on refresh', async () => {
  setLanguage('en');
  const send = vi.fn(async (body: ClientBody): Promise<ServerBody> => {
    const offset = body.case === 'rankingGet' ? body.value.offset : 0;
    return {
      case: 'rankingSnapshot',
      value: create(RankingSnapshotSchema, {
        players: [{ login: offset ? 'Second page' : 'First page' }],
        nextOffset: offset ? 0 : 20,
      }),
    };
  });
  const { container, getByRole, findByRole } = render(() => (
    <OnlineRecords mode="ranking" send={send} select={vi.fn()} />
  ));
  await findByRole('button', { name: 'First page' });
  expect(container.querySelector('ol')?.start).toBe(1);
  fireEvent.click(getByRole('button', { name: 'Next page' }));
  await findByRole('button', { name: 'Second page' });
  expect(container.querySelector('ol')?.start).toBe(21);
  await waitFor(() =>
    expect(getByRole('button', { name: 'Reload' }).hasAttribute('disabled')).toBe(false),
  );
  fireEvent.click(getByRole('button', { name: 'Reload' }));
  await findByRole('button', { name: 'First page' });
  expect(container.querySelector('ol')?.start).toBe(1);
});
