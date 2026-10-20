import { A } from '@solidjs/router';
import {
  createEffect,
  createMemo,
  createSignal,
  For,
  lazy,
  onCleanup,
  onMount,
  Show,
} from 'solid-js';
import { account, accountEpoch, api, restoreAccount } from '../../app/account';
import { avatars, icon, portrait } from '../../app/content';
import { onlineIcon } from '../../app/onlineIcons';
import { setActivityBusy, setPlaceBackdrop } from '../../app/session';
import { BackToMap } from '../../components/BackToMap';
import { Scene } from '../../components/Scene';
import { Stats } from '../../components/Stats';
import '../../app/pathCatalog';
import type {
  ChallengeReceived,
  ChatMessage,
  MailEntry,
  PublicPlayer,
  SocialSnapshot,
} from '../../generated/proto/game/v1/lobby_pb';
import { hasTranslation, t } from '../../i18n';
import { type ClientBody, OnlineClient, type ServerBody, type Status } from '../../net/client';
import { parseCommand } from '../../net/commands';
import './translations';
import '../online.css';
import type { DuelEnd, DuelSnapshot } from '../../generated/proto/game/v1/online_duel_pb';
import type { TradeSnapshot } from '../../generated/proto/game/v1/trade_pb';
import { canEnter, type Journey, requiredLevel } from './journey';
import { placeIcon, Town } from './Town';

const OnlineDuel = lazy(() => import('./OnlineDuel').then((m) => ({ default: m.OnlineDuel })));
const OnlineRecords = lazy(() =>
  import('./OnlineRecords').then((m) => ({ default: m.OnlineRecords })),
);
const Trade = lazy(() => import('./Trade').then((m) => ({ default: m.Trade })));

const rooms = ['arena', 'training', 'castle', 'portal', 'bazaar', 'shop', 'tower'];
// Every clan room shares one place: its name, description and painting (art/paintings/scenes);
// outside the rooms the player looks at the town (Town.tsx).
const place = (room: string) => (room.startsWith('clan:') ? 'clan' : room);
const scenes = import.meta.glob<string>(
  '../../assets/paintings/scenes/{arena,training,castle,portal,expedition,bazaar,shop,tower,clan}.avif',
  { eager: true, query: '?url', import: 'default' },
);
const scene = (room: string) => scenes[`../../assets/paintings/scenes/${place(room)}.avif`];

// A number with an icon in place of its text label; the label stays as the alt text and tooltip.
function Value(props: { icon: string | undefined; label: string; value: number }) {
  return (
    <span class="lobby-value">
      <img class="icon" src={props.icon} alt={props.label} title={props.label} />
      {props.value}
    </span>
  );
}
function Standing(props: { player: PublicPlayer }) {
  return (
    <>
      <Value icon={onlineIcon('level')} label={t('lobby-level-label')} value={props.player.level} />{' '}
      <Value icon={onlineIcon('tab-ranking')} label="RP" value={props.player.rating} />
    </>
  );
}
function Portrait(props: { player: PublicPlayer }) {
  return (
    <Show when={avatars.get(props.player.pathId || props.player.templateId)}>
      {(avatar) => <img class="portrait" src={portrait(avatar().key)} alt="" loading="lazy" />}
    </Show>
  );
}
const Tournaments = lazy(() =>
  import('../tournaments/Tournaments').then((module) => ({ default: module.Tournaments })),
);
const Expeditions = lazy(() =>
  import('../expeditions/Expeditions').then((module) => ({ default: module.Expeditions })),
);
const Report = lazy(() => import('../moderation/Report').then((m) => ({ default: m.Report })));
const Notices = lazy(() => import('../moderation/Notices').then((m) => ({ default: m.Notices })));

export function Lobby() {
  const client = new OnlineClient();
  const [status, setStatus] = createSignal<Status>('connecting');
  const [me, setMe] = createSignal<PublicPlayer>();
  const [room, setRoom] = createSignal('');
  // Inside the room's building, or back on the town map while still in the room (its chat and players).
  const [inside, setInside] = createSignal(false);
  const [roomMotd, setRoomMotd] = createSignal('');
  const [players, setPlayers] = createSignal<PublicPlayer[]>([]);
  const [messages, setMessages] = createSignal<ChatMessage[]>([]);
  const [social, setSocial] = createSignal<SocialSnapshot>();
  const [selected, setSelected] = createSignal<PublicPlayer>();
  const [offers, setOffers] = createSignal<ChallengeReceived[]>([]);
  const [text, setText] = createSignal('');
  const [error, setError] = createSignal('');
  const [notice, setNotice] = createSignal('');
  const [help, setHelp] = createSignal(false);
  const [journey, setJourney] = createSignal<Journey>();
  const [tutorialBusy, setTutorialBusy] = createSignal(false);
  const [chatBusy, setChatBusy] = createSignal(false);
  const [mailBusy, setMailBusy] = createSignal(false);
  const [report, setReport] = createSignal<{
    kind: 'chat' | 'player';
    target: string;
    label: string;
  }>();
  const [tab, setTab] = createSignal<'map' | 'chat' | 'social' | 'mail' | 'ranking' | 'history'>(
    'map',
  );
  const [sort, setSort] = createSignal('login');
  const [mail, setMail] = createSignal<MailEntry[]>([]);
  const [sent, setSent] = createSignal(false);
  const [cursor, setCursor] = createSignal('');
  const [now, setNow] = createSignal(Date.now());
  const [duel, setDuel] = createSignal<DuelSnapshot>();
  const [duelEnd, setDuelEnd] = createSignal<DuelEnd>();
  const [trade, setTrade] = createSignal<TradeSnapshot>();
  const [bots, setBots] = createSignal<PublicPlayer[]>([]);
  let offset = 0;
  let lastPrivate = '';
  let disposed = false;
  // A room snapshot opens its building only after the player's own choice or a reconnect;
  // coming back to the lobby from another screen starts on the town map.
  let joining = '';
  let welcomed = false;
  let rejoin = false;
  let chatLog: HTMLDivElement | undefined;
  const connected = () => status() === 'connected';
  async function loadJourney() {
    const epoch = accountEpoch();
    try {
      const result = await api<Journey>('/api/journey');
      if (!disposed && epoch === accountEpoch()) setJourney(result);
    } catch (reason) {
      showError(reason);
    }
  }
  async function advanceTutorial() {
    if (tutorialBusy() || !journey()) return;
    setTutorialBusy(true);
    const epoch = accountEpoch();
    try {
      const result = await api<Journey>('/api/journey', { step: journey()?.tutorial_step });
      if (!disposed && epoch === accountEpoch()) setJourney(result);
    } catch (reason) {
      showError(reason);
    } finally {
      setTutorialBusy(false);
    }
  }
  createEffect(() => {
    const profile = me();
    if (profile) setActivityBusy(profile.busy);
  });
  createEffect(() => setPlaceBackdrop(inside() && room() ? scene(room()) : undefined));
  const sorted = createMemo(() =>
    [...players()].sort((a, b) =>
      sort() === 'login'
        ? a.login.localeCompare(b.login)
        : sort() === 'rating'
          ? b.rating - a.rating
          : b.level - a.level,
    ),
  );
  function showError(reason: unknown) {
    if (disposed) return;
    const code = reason instanceof Error ? reason.message : 'UNAVAILABLE';
    setError(
      t(hasTranslation(`lobby-error-${code}`) ? `lobby-error-${code}` : 'lobby-error-default'),
    );
  }
  async function send(body: ClientBody) {
    setError('');
    try {
      return await client.send(body);
    } catch (reason) {
      showError(reason);
      return undefined;
    }
  }
  function receive(body: ServerBody) {
    switch (body.case) {
      case 'welcome':
        setMe(body.value.profile);
        rejoin = welcomed;
        welcomed = true;
        offset = Number(body.value.serverTimeMs) - Date.now();
        setRoom('');
        setRoomMotd('');
        setPlayers([]);
        setMessages([]);
        setOffers([]);
        setSelected(undefined);
        void loadJourney();
        // A new lobby visit starts on the town square, so its chat works before entering a building.
        if (!rejoin)
          void send({ case: 'roomJoin', value: { $typeName: 'game.v1.RoomJoin', room: 'square' } });
        if (trade())
          void send({
            case: 'tradeSnapshot',
            value: { $typeName: 'game.v1.TradeTarget', tradeId: trade()?.tradeId ?? '' },
          });
        if (duel())
          void send({
            case: 'duelSnapshot',
            value: { $typeName: 'game.v1.DuelTarget', duelId: duel()?.duelId ?? '' },
          });
        break;
      case 'tradeSnapshot':
        setTrade((previous) =>
          previous &&
          (previous.tradeId > body.value.tradeId ||
            (previous.tradeId === body.value.tradeId && previous.revision > body.value.revision))
            ? previous
            : body.value,
        );
        break;
      case 'duelSnapshot':
        setDuel((previous) =>
          previous?.duelId === body.value.duelId && previous.revision > body.value.revision
            ? previous
            : body.value,
        );
        if (body.value.result) setDuelEnd(body.value.result);
        else if (duelEnd()?.duelId !== body.value.duelId) setDuelEnd(undefined);
        break;
      case 'duelEnd':
        if (duel()?.duelId === body.value.duelId) setDuelEnd(body.value);
        void loadJourney();
        break;
      case 'trainingBots':
        setBots(body.value.bots);
        break;
      case 'roomSnapshot':
        setRoom(body.value.room);
        if (body.value.room !== 'square' && (rejoin || body.value.room === joining))
          setInside(true);
        rejoin = false;
        joining = '';
        setRoomMotd(body.value.motd);
        setPlayers(body.value.players);
        setMessages(body.value.messages);
        if (body.value.room === 'training')
          void send({ case: 'trainingBots', value: { $typeName: 'game.v1.Empty' } });
        break;
      case 'roomDelta': {
        if (body.value.room !== room()) break;
        const delta = body.value;
        setPlayers((rows) => [
          ...rows
            .filter((p) => !delta.left.includes(p.id) && !delta.joined.some((u) => u.id === p.id))
            .map((p) => delta.updated.find((u) => u.id === p.id) ?? p),
          ...delta.joined,
        ]);
        const own = delta.updated.find((p) => p.id === me()?.id);
        if (own) setMe(own);
        break;
      }
      case 'chatMessage': {
        const message = body.value;
        if (message.channel === 'private' && message.fromId !== me()?.id)
          lastPrivate = message.fromLogin;
        setMessages((rows) =>
          rows.some((m) => m.id === message.id) ? rows : [...rows, message].slice(-150),
        );
        queueMicrotask(() => chatLog?.scrollTo({ top: chatLog.scrollHeight }));
        break;
      }
      case 'socialSnapshot':
        setSocial(body.value);
        break;
      case 'playerInfo':
        setSelected(body.value.player);
        break;
      case 'mailSnapshot':
        setMail(body.value.messages);
        setCursor(body.value.nextCursor);
        setSent(body.value.sent);
        break;
      case 'challengeReceived':
        setOffers((rows) => [
          ...rows.filter((o) => o.challengeId !== body.value.challengeId),
          body.value,
        ]);
        break;
      case 'challengeUpdate':
        setOffers((rows) => rows.filter((o) => o.challengeId !== body.value.challengeId));
        setNotice(t(`lobby-challenge-${body.value.status}`));
        break;
      default:
        break;
    }
  }
  client.onMessage = receive;
  client.onStatus = setStatus;
  onMount(() => {
    if (account()?.kind !== 'account') void restoreAccount();
    const timer = window.setInterval(() => setNow(Date.now() + offset), 1000);
    onCleanup(() => clearInterval(timer));
  });
  createEffect(() => {
    if (account()?.kind === 'account') client.start();
    else {
      client.stop();
      setStatus('signed-out');
      setMe(undefined);
      setPlayers([]);
      setMessages([]);
      setSocial(undefined);
      setMail([]);
      setOffers([]);
      setDuel(undefined);
      setDuelEnd(undefined);
      setTrade(undefined);
      setBots([]);
      setJourney(undefined);
    }
  });
  onCleanup(() => {
    disposed = true;
    client.stop();
    setPlaceBackdrop(undefined);
  });
  function socialChange(player: string, ignore: boolean, add: boolean) {
    const value = { $typeName: 'game.v1.PlayerTarget' as const, player };
    return send(
      ignore
        ? add
          ? { case: 'ignoreAdd', value }
          : { case: 'ignoreRemove', value }
        : add
          ? { case: 'friendAdd', value }
          : { case: 'friendRemove', value },
    );
  }
  async function chat(event: SubmitEvent) {
    event.preventDefault();
    if (chatBusy()) return;
    setChatBusy(true);
    const submitted = text();
    try {
      const command = parseCommand(submitted, lastPrivate);
      let result: ServerBody | undefined;
      if (command.kind === 'help') {
        setHelp(true);
        setText('');
        return;
      }
      if (command.kind === 'send')
        result = await send({
          case: 'chatSend',
          value: {
            $typeName: 'game.v1.ChatSend',
            channel: command.channel,
            to: command.to,
            text: command.text,
          },
        });
      if (command.kind === 'social')
        result = await socialChange(command.player, command.relation === 'ignore', command.add);
      if (command.kind === 'list') {
        setTab('social');
        result = await send({ case: 'socialList', value: { $typeName: 'game.v1.Empty' } });
      }
      if (command.kind === 'afk') {
        result = await send({
          case: 'setAfk',
          value: { $typeName: 'game.v1.SetAfk', afk: !me()?.afk },
        });
        if (result) setMe((p) => (p ? { ...p, afk: !p.afk } : p));
      }
      if (command.kind === 'who')
        result = await send({
          case: 'whoIs',
          value: { $typeName: 'game.v1.PlayerTarget', player: command.player },
        });
      if (result && text() === submitted) setText('');
    } catch (reason) {
      showError(reason);
    } finally {
      setChatBusy(false);
    }
  }
  function loadMail(outgoing = sent(), beforeId = '') {
    return send({
      case: 'mailList',
      value: { $typeName: 'game.v1.MailList', beforeId, limit: 20, sent: outgoing },
    });
  }
  async function mailAction(id: string, remove: boolean) {
    const value = { $typeName: 'game.v1.MailTarget' as const, id };
    if (await send(remove ? { case: 'mailDelete', value } : { case: 'mailRead', value }))
      await loadMail();
  }
  async function compose(event: SubmitEvent) {
    event.preventDefault();
    if (mailBusy()) return;
    setMailBusy(true);
    const form = event.currentTarget as HTMLFormElement;
    const data = new FormData(form);
    if (
      await send({
        case: 'mailSend',
        value: {
          $typeName: 'game.v1.MailSend',
          to: String(data.get('to')).trim(),
          text: String(data.get('text')).trim(),
        },
      })
    ) {
      form.reset();
      setNotice(t('lobby-mail-sent'));
      if (sent()) await loadMail(true);
    }
    setMailBusy(false);
  }
  const time = (value: bigint) =>
    new Date(Number(value)).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  function enter(name: string) {
    if (!canEnter(journey(), name)) {
      setError(t('lobby-location-locked', { level: requiredLevel(journey(), name) }));
      return;
    }
    if (name === room()) setInside(true);
    else {
      joining = name;
      void send({ case: 'roomJoin', value: { $typeName: 'game.v1.RoomJoin', room: name } });
    }
  }
  // A challenge waiting for the player's answer marks the arena on the map.
  const challenged = () =>
    offers().some((offer) => offer.to?.id === me()?.id && Number(offer.expiresAtMs) > now());
  const inRoom = () => tab() === 'map' && inside() && Boolean(room());
  return (
    <section
      class="lobby-page"
      classList={{ 'lobby-map-mode': tab() === 'map' && !inside() && !duel() && !trade() }}
    >
      {/* The player's bar, as in mobile games: portrait, level and rating, challenge settings. */}
      <div class="lobby-hud">
        <h1 class="visually-hidden">{t('lobby')}</h1>
        <Show when={me()}>
          {(self) => (
            <>
              <Portrait player={self()} />
              <strong>{self().login}</strong>
              <span>
                <Standing player={self()} />
              </span>
              <Show when={journey()}>
                {(progress) => (
                  <span class="lobby-account-level">
                    {t('lobby-account-level', { level: progress().level })}
                  </span>
                )}
              </Show>
              <label>
                <span class="lobby-hud-label">{t('lobby-challenges')}</span>
                <select
                  aria-label={t('lobby-challenges')}
                  value={self().challengeFilter}
                  disabled={!connected()}
                  onChange={async (e) => {
                    const filter = e.currentTarget.value;
                    if (
                      await send({
                        case: 'setChallengeFilter',
                        value: { $typeName: 'game.v1.SetChallengeFilter', filter },
                      })
                    )
                      setMe((p) => (p ? { ...p, challengeFilter: filter } : p));
                  }}
                >
                  <option value="all">{t('lobby-all')}</option>
                  <option value="level_pm2">±2 {t('lobby-level-label')}</option>
                  <option value="none">{t('lobby-none')}</option>
                </select>
              </label>
              <button
                type="button"
                disabled={!connected()}
                aria-pressed={self().afk}
                onClick={async () => {
                  if (
                    await send({
                      case: 'setAfk',
                      value: { $typeName: 'game.v1.SetAfk', afk: !self().afk },
                    })
                  )
                    setMe((p) => (p ? { ...p, afk: !p.afk } : p));
                }}
              >
                <img class="icon" src={onlineIcon('afk')} alt="" />
                AFK
              </button>
            </>
          )}
        </Show>
        <p class="lobby-connection" role="status" data-testid="connection-status">
          {t(`lobby-status-${status()}`)}
        </p>
      </div>
      <Show
        when={status() === 'replaced' || status() === 'incompatible' || status() === 'signed-out'}
      >
        <p>{t(status() === 'incompatible' ? 'lobby-update' : 'lobby-login-needed')}</p>
        <A href="/account">{t('auth-account')}</A>{' '}
        <button type="button" onClick={() => client.restart()}>
          {t('lobby-reconnect')}
        </button>
      </Show>
      <Show when={error()}>
        <p class="lobby-error" role="alert">
          {error()}
        </p>
      </Show>
      <Show when={notice()}>
        <p role="status">{notice()}</p>
      </Show>
      <Show when={duel()}>
        {(active) => (
          <OnlineDuel
            snapshot={active()}
            end={duelEnd()}
            connected={connected()}
            now={now()}
            send={send}
            tutorial={(journey()?.tutorial_step ?? 3) < 3}
            close={() => {
              setDuel(undefined);
              setDuelEnd(undefined);
              void loadJourney();
            }}
          />
        )}
      </Show>
      <Show when={trade()}>
        {(value) => (
          <Trade
            snapshot={value()}
            user={me()?.id ?? ''}
            connected={connected()}
            now={now()}
            send={send}
            close={() => setTrade(undefined)}
          />
        )}
      </Show>
      <Show when={me()}>
        {(self) => (
          <>
            <Show when={!self().verified}>
              <p class="lobby-hint">{t('lobby-unverified')}</p>
            </Show>
            <div class="lobby-layout">
              <div class="lobby-main">
                <nav class="lobby-tabs lobby-menu" aria-label={t('lobby')}>
                  <For each={['map', 'chat', 'social', 'mail', 'ranking', 'history'] as const}>
                    {(value) => (
                      <button
                        type="button"
                        class="plaque"
                        aria-pressed={tab() === value}
                        onClick={() => {
                          setTab(value);
                          if (value === 'map') setInside(false);
                          if (value === 'mail') void loadMail();
                        }}
                      >
                        <img
                          class="icon"
                          src={onlineIcon(`tab-${value === 'chat' ? 'social' : value}`)}
                          alt=""
                        />
                        {t(`lobby-tab-${value}`)}
                      </button>
                    )}
                  </For>
                </nav>
                <Show when={tab() === 'map'}>
                  <Show when={!inRoom() && journey()}>
                    {(progress) => (
                      <div class="lobby-journey">
                        <Show
                          when={progress().tutorial_step < 3}
                          fallback={
                            <p>
                              {progress().next_xp === null
                                ? t('lobby-account-complete')
                                : t('lobby-account-progress', {
                                    xp: progress().xp,
                                    next: progress().next_xp ?? progress().xp,
                                  })}
                            </p>
                          }
                        >
                          <strong>
                            {t('lobby-tutorial-step', { step: progress().tutorial_step + 1 })}
                          </strong>
                          <p>{t(`lobby-tutorial-${progress().tutorial_step}`)}</p>
                          <Show
                            when={progress().tutorial_step < 2}
                            fallback={
                              <button
                                type="button"
                                class="primary"
                                disabled={!connected()}
                                onClick={() => enter('training')}
                              >
                                {t('lobby-tutorial-train')}
                              </button>
                            }
                          >
                            <button
                              type="button"
                              class="primary"
                              disabled={tutorialBusy()}
                              onClick={() => void advanceTutorial()}
                            >
                              {t('lobby-tutorial-next')}
                            </button>
                          </Show>
                          <Show when={progress().tutorial_step === 1}>
                            <A href="/collection">{t('lobby-tutorial-deck')}</A>
                          </Show>
                        </Show>
                      </div>
                    )}
                  </Show>
                  <Show
                    when={inRoom()}
                    fallback={
                      <Town
                        connected={connected()}
                        journey={journey()}
                        clan={self().clanId}
                        alert={challenged() ? 'arena' : ''}
                        enter={enter}
                      />
                    }
                  >
                    {/* Inside a building: its painting, the way back and a quick way to the others. */}
                    <Scene src={scene(room())} about={t(`lobby-room-${place(room())}-about`)}>
                      <h2 class="plaque">{t(`lobby-room-${place(room())}`)}</h2>
                      <BackToMap onClick={() => setInside(false)} />
                    </Scene>
                    <nav class="lobby-travel" aria-label={t('lobby-travel')}>
                      <For each={self().clanId ? [...rooms, `clan:${self().clanId}`] : rooms}>
                        {(name) => (
                          <button
                            type="button"
                            class="plaque"
                            aria-current={room() === name ? 'location' : undefined}
                            disabled={!connected() || !canEnter(journey(), name)}
                            onClick={() => enter(name)}
                          >
                            <img class="icon" src={placeIcon(place(name))} alt="" />
                            {t(`lobby-room-${place(name)}`)}
                          </button>
                        )}
                      </For>
                    </nav>
                  </Show>
                </Show>
                <Show when={tab() === 'ranking'}>
                  <OnlineRecords mode="ranking" send={send} select={setSelected} />
                </Show>
                <Show when={tab() === 'history'}>
                  <OnlineRecords mode="history" send={send} select={setSelected} />
                </Show>
                <Show when={inRoom() && room() === 'shop'}>
                  <A class="button primary lobby-enter" href="/store">
                    {t('store')}
                  </A>
                </Show>
                <Show when={inRoom() && room() === 'castle'}>
                  <div hidden={Boolean(duel() || trade())}>
                    <Tournaments
                      send={send}
                      connected={connected()}
                      avatar={self().avatarId}
                      active={!duel() && !trade()}
                    />
                  </div>
                </Show>
                <Show when={inRoom() && (room() === 'tower' || self().clanId)}>
                  <A class="button secondary lobby-enter" href="/clans">
                    {t('clans')}
                  </A>
                </Show>
                {/* Kept mounted while the player stays in the portal, so a chosen expedition survives a look at the map or mail. */}
                <Show when={room() === 'portal' || room() === 'expedition'}>
                  <div hidden={Boolean(duel() || trade()) || !inRoom()}>
                    <Expeditions
                      avatar={self().avatarId}
                      verified={self().verified}
                      active={!duel() && !trade()}
                      connected={connected()}
                      room={room()}
                      send={send}
                    />
                  </div>
                </Show>
                <Show when={inRoom() && room() === 'training'}>
                  <section class="lobby-panel">
                    <h2>{t('online-bots')}</h2>
                    <p>{t('online-training-help')}</p>
                    <button
                      type="button"
                      disabled={!connected()}
                      onClick={() =>
                        void send({ case: 'trainingBots', value: { $typeName: 'game.v1.Empty' } })
                      }
                    >
                      {t('refresh')}
                    </button>
                    <div class="online-bots">
                      <For each={(journey()?.tutorial_step ?? 3) < 3 ? bots().slice(0, 1) : bots()}>
                        {(bot) => (
                          <button
                            type="button"
                            disabled={!connected() || !!me()?.busy}
                            onClick={() =>
                              void send({
                                case: 'trainingChallengeBot',
                                value: { $typeName: 'game.v1.TrainingChallengeBot', botId: bot.id },
                              })
                            }
                          >
                            <Portrait player={bot} />
                            {bot.login}
                          </button>
                        )}
                      </For>
                    </div>
                  </section>
                </Show>
                <Show when={tab() === 'social'}>
                  <div class="lobby-panel">
                    <h2>{t('lobby-friends')}</h2>
                    <For each={social()?.friends}>
                      {(p) => (
                        <div class="lobby-social-row">
                          <button
                            class="quiet"
                            type="button"
                            onClick={() =>
                              void send({
                                case: 'whoIs',
                                value: { $typeName: 'game.v1.PlayerTarget', player: p.login },
                              })
                            }
                          >
                            {p.login} · {t(p.online ? 'lobby-online' : 'lobby-offline')}
                          </button>
                          <button
                            type="button"
                            disabled={!connected()}
                            onClick={() => void socialChange(p.login, false, false)}
                          >
                            {t('lobby-remove')}
                          </button>
                        </div>
                      )}
                    </For>
                    <h2>{t('lobby-ignores')}</h2>
                    <For each={social()?.ignores}>
                      {(p) => (
                        <div class="lobby-social-row">
                          <span>{p.login}</span>
                          <button
                            type="button"
                            disabled={!connected()}
                            onClick={() => void socialChange(p.login, true, false)}
                          >
                            {t('lobby-remove')}
                          </button>
                        </div>
                      )}
                    </For>
                    <p>{t('lobby-social-help')}</p>
                  </div>
                </Show>
                <Show when={tab() === 'mail'}>
                  <div class="lobby-panel">
                    <h2>{t('lobby-tab-mail')}</h2>
                    <div class="lobby-tabs">
                      <button
                        type="button"
                        disabled={!connected()}
                        aria-pressed={!sent()}
                        onClick={() => void loadMail(false)}
                      >
                        {t('lobby-inbox')}
                      </button>
                      <button
                        type="button"
                        disabled={!connected()}
                        aria-pressed={sent()}
                        onClick={() => void loadMail(true)}
                      >
                        {t('lobby-sent')}
                      </button>
                    </div>
                    <For each={mail()} fallback={<p>{t('lobby-empty')}</p>}>
                      {(m) => (
                        <article class="lobby-letter">
                          <strong>{sent() ? m.toLogin : m.fromLogin}</strong>{' '}
                          <time>{time(m.atMs)}</time>
                          <p>{m.text}</p>
                          <Show when={!sent() && !m.read}>
                            <button type="button" onClick={() => void mailAction(m.id, false)}>
                              {t('lobby-read')}
                            </button>
                          </Show>{' '}
                          <button type="button" onClick={() => void mailAction(m.id, true)}>
                            {t('lobby-remove')}
                          </button>
                        </article>
                      )}
                    </For>
                    <Show when={cursor()}>
                      <button
                        type="button"
                        disabled={!connected()}
                        onClick={() => void loadMail(sent(), cursor())}
                      >
                        {t('lobby-older')}
                      </button>
                    </Show>
                    <form class="lobby-compose" onSubmit={(event) => void compose(event)}>
                      <label>
                        {t('lobby-recipient')}
                        <input
                          name="to"
                          required
                          minLength={3}
                          maxLength={20}
                          pattern={String.raw`[A-Za-z0-9_\-]+`}
                        />
                      </label>
                      <label>
                        {t('lobby-message')}
                        <textarea name="text" required maxLength={1000} rows={3} />
                      </label>
                      <button type="submit" disabled={!connected() || mailBusy()}>
                        {t('lobby-send-mail')}
                      </button>
                    </form>
                  </div>
                </Show>
                <section class="lobby-chat" classList={{ 'lobby-chat-active': tab() === 'chat' }}>
                  <Notices bulletin />
                  <Show when={roomMotd()}>
                    <p class="lobby-notice">{roomMotd()}</p>
                  </Show>
                  <div class="lobby-chat-heading">
                    <h2>
                      {t('lobby-chat')} · {t(`lobby-room-${place(room() || 'square')}`)}
                    </h2>
                    <label>
                      {t('lobby-chat-filter')}
                      <select
                        disabled={!connected()}
                        onChange={(e) =>
                          void send({
                            case: 'setChatFilter',
                            value: {
                              $typeName: 'game.v1.SetChatFilter',
                              filter: e.currentTarget.value,
                            },
                          })
                        }
                      >
                        <option value="all">{t('lobby-all')}</option>
                        <option value="clan_private">{t('lobby-clan-private')}</option>
                        <option value="private">{t('lobby-private')}</option>
                      </select>
                    </label>
                  </div>
                  <div
                    ref={chatLog}
                    class="lobby-log"
                    role="log"
                    aria-label={t('lobby-messages')}
                    aria-live="polite"
                  >
                    <For each={messages()}>
                      {(m) => (
                        <p classList={{ private: m.channel === 'private' }}>
                          <time>{time(m.atMs)}</time>{' '}
                          <button
                            class="quiet"
                            type="button"
                            onClick={() =>
                              void send({
                                case: 'whoIs',
                                value: { $typeName: 'game.v1.PlayerTarget', player: m.fromLogin },
                              })
                            }
                          >
                            {m.fromLogin}
                          </button>
                          <Show when={m.channel !== 'room'}>
                            <small> [{t(`lobby-channel-${m.channel}`)}]</small>
                          </Show>
                          : <span>{m.text}</span>
                          <button
                            type="button"
                            class="quiet"
                            aria-label={t('lobby-report-message', { login: m.fromLogin })}
                            onClick={() =>
                              setReport({
                                kind: 'chat',
                                target: m.id,
                                label: `${m.fromLogin}: ${m.text}`,
                              })
                            }
                          >
                            {t('lobby-report')}
                          </button>
                        </p>
                      )}
                    </For>
                  </div>
                  <form class="lobby-chat-form" onSubmit={(e) => void chat(e)}>
                    <input
                      aria-label={t('lobby-message')}
                      value={text()}
                      onInput={(e) => setText(e.currentTarget.value)}
                      maxLength={330}
                      disabled={!connected() || !room()}
                      enterkeyhint="send"
                      placeholder={t('lobby-chat-placeholder')}
                    />
                    <button
                      type="submit"
                      class="primary"
                      disabled={!connected() || !room() || chatBusy() || !text().trim()}
                    >
                      {t('lobby-send')}
                    </button>
                    <button type="button" aria-expanded={help()} onClick={() => setHelp(!help())}>
                      /help
                    </button>
                  </form>
                  <Show when={help()}>
                    <p class="lobby-help">{t('lobby-help')}</p>
                  </Show>
                </section>
              </div>
              <aside class="lobby-sidebar">
                <div class="lobby-panel">
                  <div class="lobby-chat-heading">
                    <h2>{t('lobby-players', { count: players().length })}</h2>
                    <select
                      aria-label={t('lobby-sort')}
                      value={sort()}
                      onChange={(e) => setSort(e.currentTarget.value)}
                    >
                      <option value="login">{t('lobby-name')}</option>
                      <option value="rating">RP</option>
                      <option value="level">{t('lobby-level-label')}</option>
                    </select>
                  </div>
                  <ul class="lobby-player-list">
                    <For each={sorted()}>
                      {(p) => (
                        <li>
                          <button
                            type="button"
                            onClick={() =>
                              void send({
                                case: 'whoIs',
                                value: { $typeName: 'game.v1.PlayerTarget', player: p.login },
                              })
                            }
                          >
                            <Portrait player={p} />
                            <span>
                              {p.login} {p.mentor ? ` · ${t('lobby-mentor')}` : ''}{' '}
                              <Show when={p.afk}>
                                <img
                                  class="icon"
                                  src={onlineIcon('afk')}
                                  alt={t('lobby-afk')}
                                  title={t('lobby-afk')}
                                />
                              </Show>
                              <Show when={p.busy}>
                                <img
                                  class="icon"
                                  src={icon('school-combat')}
                                  alt={t('lobby-busy')}
                                  title={t('lobby-busy')}
                                />
                              </Show>
                            </span>
                            <small>
                              <Standing player={p} />
                            </small>
                            <small>
                              {t(`lobby-${p.title}`)} ·{' '}
                              {avatars.get(p.pathId || p.templateId)
                                ? t(
                                    `${avatars.get(p.pathId || p.templateId)?.key.replace('.', '-')}-name`,
                                  )
                                : '—'}
                            </small>
                          </button>
                        </li>
                      )}
                    </For>
                  </ul>
                </div>
                <Show when={selected()} keyed>
                  {(p) => (
                    <div class="lobby-panel lobby-profile">
                      <button
                        class="lobby-close"
                        type="button"
                        aria-label={t('lobby-close')}
                        onClick={() => setSelected(undefined)}
                      >
                        ×
                      </button>
                      <Show when={avatars.get(p.pathId || p.templateId)}>
                        {(avatar) => (
                          <img width="80" height="80" alt="" src={portrait(avatar().key)} />
                        )}
                      </Show>
                      <h2>{p.login}</h2>
                      <Show when={p.mentor}>
                        <p>{t('lobby-mentor')}</p>
                      </Show>
                      <Show when={p.id !== self().id}>
                        <button
                          type="button"
                          onClick={() =>
                            setReport({ kind: 'player', target: p.id, label: p.login })
                          }
                        >
                          {t('lobby-report')}
                        </button>
                      </Show>
                      <p>
                        <Standing player={p} /> · #{p.rank || '—'}
                      </p>
                      <Show when={p.title}>
                        <p>{t(`lobby-${p.title}`)}</p>
                      </Show>
                      <Show when={p.clanName}>
                        <p>{p.clanName}</p>
                      </Show>
                      <p>{p.description}</p>
                      <Stats values={p.stats} />
                      <Show when={p.id !== self().id}>
                        <div class="lobby-actions">
                          <button
                            type="button"
                            disabled={
                              !connected() || room() !== 'arena' || p.busy || self().busy || p.afk
                            }
                            onClick={() =>
                              void send({
                                case: 'challengeSend',
                                value: { $typeName: 'game.v1.ChallengeSend', target: p.login },
                              })
                            }
                          >
                            {t('lobby-challenge')}
                          </button>
                          <button
                            type="button"
                            disabled={!connected()}
                            onClick={() => {
                              setText(`/t ${p.login}, `);
                            }}
                          >
                            {t('lobby-private')}
                          </button>
                          <button
                            type="button"
                            disabled={
                              !connected() ||
                              room() !== 'bazaar' ||
                              !p.verified ||
                              !self().verified ||
                              p.busy ||
                              self().busy ||
                              p.afk
                            }
                            onClick={() =>
                              void send({
                                case: 'tradeInvite',
                                value: { $typeName: 'game.v1.TradeInvite', target: p.login },
                              })
                            }
                          >
                            {t('trade-title')}
                          </button>
                          <button
                            type="button"
                            disabled={!connected()}
                            onClick={() => void socialChange(p.login, false, true)}
                          >
                            {t('lobby-add-friend')}
                          </button>
                          <button
                            type="button"
                            disabled={!connected()}
                            onClick={() => void socialChange(p.login, true, true)}
                          >
                            {t('lobby-ignore')}
                          </button>
                        </div>
                      </Show>
                    </div>
                  )}
                </Show>
                <For each={offers()}>
                  {(offer) => (
                    <div class="lobby-panel lobby-offer">
                      <h2>{t('lobby-challenge')}</h2>
                      <p>
                        {offer.from?.login} → {offer.to?.login}
                      </p>
                      <Show when={offer.from} keyed>
                        {(p) => (
                          <>
                            <Show when={avatars.get(p.pathId || p.templateId)}>
                              {(avatar) => (
                                <img width="64" height="64" alt="" src={portrait(avatar().key)} />
                              )}
                            </Show>
                            <p>
                              <Standing player={p} /> · #{p.rank || '—'}
                            </p>
                            <p>
                              {t(`lobby-${p.title}`)} {p.clanName}
                            </p>
                            <Stats values={p.stats} />
                            <p>{p.description}</p>
                          </>
                        )}
                      </Show>
                      <p>{Math.max(0, Math.ceil((Number(offer.expiresAtMs) - now()) / 1000))} s</p>
                      <Show
                        when={offer.to?.id === self().id}
                        fallback={
                          <button
                            type="button"
                            disabled={!connected()}
                            onClick={() =>
                              void send({
                                case: 'challengeCancel',
                                value: {
                                  $typeName: 'game.v1.ChallengeCancel',
                                  challengeId: offer.challengeId,
                                },
                              })
                            }
                          >
                            {t('lobby-cancel')}
                          </button>
                        }
                      >
                        <button
                          type="button"
                          disabled={!connected() || Number(offer.expiresAtMs) <= now()}
                          onClick={() =>
                            void send({
                              case: 'challengeRespond',
                              value: {
                                $typeName: 'game.v1.ChallengeRespond',
                                challengeId: offer.challengeId,
                                accept: true,
                              },
                            })
                          }
                        >
                          {t('lobby-accept')}
                        </button>{' '}
                        <button
                          type="button"
                          disabled={!connected()}
                          onClick={() =>
                            void send({
                              case: 'challengeRespond',
                              value: {
                                $typeName: 'game.v1.ChallengeRespond',
                                challengeId: offer.challengeId,
                                accept: false,
                              },
                            })
                          }
                        >
                          {t('lobby-decline')}
                        </button>
                      </Show>
                    </div>
                  )}
                </For>
              </aside>
            </div>
          </>
        )}
      </Show>
      <Show when={report()}>
        {(value) => <Report {...value()} close={() => setReport(undefined)} />}
      </Show>
    </section>
  );
}
