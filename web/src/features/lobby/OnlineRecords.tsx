import { createSignal, For, onMount, Show } from 'solid-js';
import type { PublicPlayer } from '../../generated/proto/game/v1/lobby_pb';
import type {
  MatchHistory,
  MatchReplayData,
  RankingSnapshot,
} from '../../generated/proto/game/v1/online_duel_pb';
import { t } from '../../i18n';
import type { ClientBody, ServerBody } from '../../net/client';
import { OnlineDuel } from './OnlineDuel';

export function OnlineRecords(props: {
  mode: 'ranking' | 'history';
  send: (body: ClientBody) => Promise<ServerBody | undefined>;
  select: (player: PublicPlayer) => void;
}) {
  const [ranking, setRanking] = createSignal<RankingSnapshot>();
  const [history, setHistory] = createSignal<MatchHistory>();
  const [replay, setReplay] = createSignal<MatchReplayData>();
  const [round, setRound] = createSignal(0);
  const [loading, setLoading] = createSignal(false);
  let page = 0;
  async function load(next = false) {
    setLoading(true);
    try {
      if (props.mode === 'ranking') {
        const offset = next ? (ranking()?.nextOffset ?? 0) : 0;
        const reply = await props.send({
          case: 'rankingGet',
          value: { $typeName: 'game.v1.RankingGet', offset, limit: 20 },
        });
        if (reply?.case === 'rankingSnapshot') {
          setRanking(reply.value);
          page = offset;
        }
      } else {
        const reply = await props.send({
          case: 'matchHistoryGet',
          value: {
            $typeName: 'game.v1.MatchHistoryGet',
            player: '',
            beforeId: next ? (history()?.nextCursor ?? '') : '',
            limit: 20,
          },
        });
        if (reply?.case === 'matchHistory') setHistory(reply.value);
      }
    } finally {
      setLoading(false);
    }
  }
  onMount(() => void load());
  async function watch(id: string) {
    const response = await props.send({
      case: 'matchReplay',
      value: { $typeName: 'game.v1.MatchReplay', matchId: id },
    });
    if (response?.case === 'matchReplayData') {
      setReplay(response.value);
      setRound(0);
    }
  }
  return (
    <section class="lobby-panel">
      <h2>{t(`online-${props.mode}`)}</h2>
      <button type="button" disabled={loading()} onClick={() => void load()}>
        {t('refresh')}
      </button>
      <Show when={props.mode === 'ranking'}>
        <ol start={page + 1} class="online-ranking">
          <For each={ranking()?.players}>
            {(p) => (
              <li>
                <button type="button" class="quiet" onClick={() => props.select(p)}>
                  {p.login}
                </button>{' '}
                · {p.rating} RP · {t('lobby-level', { level: p.level })}
              </li>
            )}
          </For>
        </ol>
        <Show when={ranking()?.nextOffset}>
          <button type="button" disabled={loading()} onClick={() => void load(true)}>
            {t('online-next')}
          </button>
        </Show>
      </Show>
      <Show when={props.mode === 'history'}>
        <For each={history()?.matches} fallback={<p>{t('lobby-empty')}</p>}>
          {(m) => (
            <article class="online-match">
              <p>
                <time>{new Date(Number(m.atMs)).toLocaleString()}</time> ·{' '}
                {t(m.kind === 'training' ? 'training' : 'lobby-room-arena')}
              </p>
              <strong>
                {m.first} / {m.second}
              </strong>
              <p>
                {m.winner === 0 ? t('online-draw') : m.winner === 1 ? m.first : m.second} ·{' '}
                {t(`online-reason-${m.reason}`)}
              </p>
              <button type="button" onClick={() => void watch(m.id)}>
                {t('online-replay')}
              </button>
            </article>
          )}
        </For>
        <Show when={history()?.nextCursor}>
          <button type="button" disabled={loading()} onClick={() => void load(true)}>
            {t('lobby-older')}
          </button>
        </Show>
      </Show>
      <Show when={replay()}>
        {(data) => (
          <div>
            <label>
              {t('online-replay-position')}{' '}
              <input
                type="range"
                min="0"
                max={data().rounds.length - 1}
                value={round()}
                onInput={(e) => setRound(Number(e.currentTarget.value))}
              />
            </label>
            <Show when={data().rounds[round()]}>
              {(snapshot) => (
                <OnlineDuel
                  snapshot={snapshot()}
                  connected={false}
                  now={0}
                  replay
                  send={props.send}
                  close={() => setReplay(undefined)}
                />
              )}
            </Show>
          </div>
        )}
      </Show>
    </section>
  );
}
