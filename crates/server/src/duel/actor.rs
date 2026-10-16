use super::{
    Error, Result, Settings, journal,
    record::{ExternalEnd, Record},
    result,
};
use engine::{EventBuf, Phase, Side};
use protocol::{
    Message as _,
    game::v1::{self as pb, server_message::Body},
};
use sha2::{Digest as _, Sha256};
use sqlx::PgPool;
use std::time::Duration;
use tokio::{
    sync::{Semaphore, mpsc, oneshot},
    time::Instant,
};
use uuid::Uuid;

static AI_SLOTS: Semaphore = Semaphore::const_new(4);
pub enum Action {
    Snapshot,
    Preview { round: u32, mv: pb::DuelMove },
    Submit { round: u32, mv: pb::DuelMove },
    Forfeit,
}
pub enum Command {
    Request {
        user: Uuid,
        action: Action,
        reply: oneshot::Sender<Result<Body>>,
    },
    Connection(Uuid, bool),
    Stop(oneshot::Sender<()>),
}
pub enum Notice {
    Send(Uuid, Body),
    ProfilesChanged(Vec<Uuid>),
    Recovered(Uuid, Vec<Uuid>),
    Finished(Uuid, Vec<Uuid>),
}
pub struct Actor {
    pub db: PgPool,
    pub instance: Uuid,
    pub record: Record,
    pub settings: Settings,
    pub notices: mpsc::Sender<Notice>,
    pub receiver: mpsc::Receiver<Command>,
}
impl Actor {
    pub async fn run(mut self) -> Result<()> {
        let (mut state, mut events) = self.record.rebuild()?;
        let mut tick = tokio::time::interval(self.settings.tick);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut renewed = Instant::now();
        let mut dirty = true;
        let mut settled = false;
        let mut connections: [Option<(bool, Option<i64>)>; 2] = [None, None];
        loop {
            if !state.is_finished() {
                for side in Side::BOTH {
                    let i = side.index();
                    if let Some((connected, grace)) = connections[i] {
                        let mut next = self.record.clone();
                        next.connected[i] = connected;
                        next.grace_until[i] = grace;
                        match self.persist(next, &state).await {
                            Ok(()) => {
                                connections[i] = None;
                                dirty = true;
                            }
                            Err(Error("LEASE_LOST")) => return Err(Error("LEASE_LOST")),
                            Err(_) => {}
                        }
                    }
                }
            }
            if state.is_finished() {
                match result::settle(&self.db, self.instance, &self.record, &state).await {
                    Ok(rewards) => {
                        settled = true;
                        let mut sent = true;
                        for side in Side::BOTH {
                            if let Some(user) = self.record.seats[side.index()].user {
                                let result = pb::DuelEnd {
                                    duel_id: self.record.id.to_string(),
                                    outcome: Some(protocol::duel::outcome(
                                        match state.phase() {
                                            Phase::Finished(o) => o,
                                            _ => unreachable!(),
                                        },
                                        side,
                                    )),
                                    xp_gained: rewards.xp[side.index()],
                                    rating_delta: rewards.delta[side.index()],
                                    rating_new: rewards.rating[side.index()],
                                    seed: self.record.seed.to_vec(),
                                    unlocked_template: rewards.unlock[side.index()]
                                        .map_or(0, u32::from),
                                    ducats_gained: rewards.ducats[side.index()],
                                };
                                let mut snapshot = self.snapshot(&state, &events, side)?;
                                snapshot.result = Some(result.clone());
                                sent &= self
                                    .notices
                                    .try_send(Notice::Send(
                                        user,
                                        Body::DuelSnapshot(Box::new(snapshot)),
                                    ))
                                    .is_ok();
                                sent &= self
                                    .notices
                                    .try_send(Notice::Send(user, Body::DuelEnd(result)))
                                    .is_ok();
                            }
                        }
                        if sent {
                            break;
                        }
                    }
                    Err(Error("LEASE_LOST")) => break,
                    Err(_) => {}
                }
            } else if dirty {
                dirty = !self.publish(&state, &events);
            }
            tokio::select! {
                command=self.receiver.recv()=>match command {
                    Some(Command::Request{user,action,reply})=>{
                        if reply.is_closed(){continue;}
                        let revision=self.record.revision;
                        let response=self.request(user,action,&mut state,&mut events).await;
                        let lost=matches!(&response,Err(Error("LEASE_LOST")));
                        let _=reply.send(response);dirty|=revision!=self.record.revision;
                        if lost {break;}
                    },
                    Some(Command::Connection(user,connected))=>{
                        if let Ok(side)=self.record.side(user) {
                            let i=side.index();
                            if self.record.connected[i]!=connected || connections[i].is_some() {
                                connections[i]=Some((connected,(!connected).then(||crate::online::now_ms()+self.settings.reconnect_ms())));
                            }
                            if connected { dirty=true; }
                        }
                    },
                    Some(Command::Stop(reply))=>{let _=journal::release_lease(&self.db,self.record.id,self.instance).await;let _=reply.send(());break;},
                    None=>break,
                },
                _=tick.tick()=>{
                    if renewed.elapsed()>=self.settings.lease/3 && !settled {
                        if journal::renew(&self.db,self.record.id,self.instance,self.settings.lease_ms()).await.is_err(){break;}
                        renewed=Instant::now();
                    }
                    if !state.is_finished() {
                        match self.advance(&mut state,&mut events).await {Ok(changed)=>dirty|=changed,Err(Error("LEASE_LOST"))=>break,Err(_)=>{}}
                    }
                }
            }
        }
        Ok(())
    }
    fn snapshot(
        &self,
        state: &engine::DuelState,
        events: &EventBuf,
        side: Side,
    ) -> Result<pb::DuelSnapshot> {
        snapshot(&self.record, state, events, side)
    }
    fn publish(&self, state: &engine::DuelState, events: &EventBuf) -> bool {
        let mut sent = true;
        for side in Side::BOTH {
            if let Some(user) = self.record.seats[side.index()].user
                && let Ok(snapshot) = self.snapshot(state, events, side)
            {
                sent &= self
                    .notices
                    .try_send(Notice::Send(user, Body::DuelSnapshot(Box::new(snapshot))))
                    .is_ok();
            }
        }
        sent
    }
    async fn persist(&mut self, mut next: Record, state: &engine::DuelState) -> Result<()> {
        next.revision = self
            .record
            .revision
            .checked_add(1)
            .ok_or(Error("INTERNAL"))?;
        journal::save(&self.db, self.instance, &self.record, &next, state).await?;
        self.record = next;
        Ok(())
    }
    async fn request(
        &mut self,
        user: Uuid,
        action: Action,
        state: &mut engine::DuelState,
        events: &mut EventBuf,
    ) -> Result<Body> {
        let side = self.record.side(user)?;
        let i = side.index();
        match action {
            Action::Snapshot => Ok(Body::DuelSnapshot(Box::new(
                self.snapshot(state, events, side)?,
            ))),
            Action::Preview { round, mv } => {
                if state.is_finished() || round != u32::from(state.round()) {
                    return Err(Error("CONFLICT"));
                }
                let mv = protocol::duel::decode_move(&mv).map_err(|_| Error("INVALID_MOVE"))?;
                let seen = engine::view(state, side);
                let content = engine_content::content();
                Ok(Body::DuelMovePreview(pb::DuelMovePreview {
                    preview: Some(protocol::duel::preview(seen.preview(&content, &mv))),
                    cast_options: (0..seen.hand.len())
                        .map(|index| {
                            protocol::duel::preview(seen.preview(
                                &content,
                                &engine::Move {
                                    card: engine::CardAction::Cast(index as u8),
                                    ..mv
                                },
                            ))
                        })
                        .collect(),
                }))
            }
            Action::Submit { round, mv } => {
                if state.is_finished()
                    || round != u32::from(state.round())
                    || self.record.pending[i].is_some()
                {
                    return Err(Error("CONFLICT"));
                }
                if crate::online::now_ms() >= self.record.deadline_ms {
                    return Err(Error("TIMEOUT"));
                }
                let decoded =
                    protocol::duel::decode_move(&mv).map_err(|_| Error("INVALID_MOVE"))?;
                engine::validate_move(state, &engine_content::content(), side, &decoded).map_err(
                    |e| {
                        Error(match e {
                            engine::MoveError::WrongPhase => "CONFLICT",
                            _ => "INVALID_MOVE",
                        })
                    },
                )?;
                let mut next = self.record.clone();
                next.pending[i] = Some(mv.encode_to_vec());
                next.timeouts[i] = 0;
                next.deadline_ms = next
                    .deadline_ms
                    .min(crate::online::now_ms() + self.settings.opponent_ms());
                let started = Instant::now();
                let previous_round = state.round();
                let (new_state, new_events) = self.resolve(&mut next, false).await?;
                self.persist(next, &new_state).await?;
                if new_state.round() != previous_round || new_state.is_finished() {
                    crate::telemetry::metrics::ROUNDS.observe(started.into());
                }
                *state = new_state;
                *events = new_events;
                Ok(Body::DuelMoveAccepted(pb::DuelMoveAccepted {
                    duel_id: self.record.id.to_string(),
                    round,
                }))
            }
            Action::Forfeit => {
                if state.is_finished() {
                    return Err(Error("CONFLICT"));
                }
                let mut next = self.record.clone();
                next.ended = Some(ExternalEnd {
                    loser: Some(i as u8),
                    cause: 0,
                });
                let (new_state, new_events) = next.rebuild()?;
                self.persist(next, &new_state).await?;
                *state = new_state;
                *events = new_events;
                Ok(crate::online::ok())
            }
        }
    }
    async fn advance(
        &mut self,
        state: &mut engine::DuelState,
        events: &mut EventBuf,
    ) -> Result<bool> {
        let now = crate::online::now_ms();
        let mut next = self.record.clone();
        let disconnected: Vec<_> = (0..2)
            .filter(|i| next.grace_until[*i].is_some_and(|at| at <= now))
            .collect();
        if !disconnected.is_empty() {
            next.ended = Some(ExternalEnd {
                loser: if disconnected.len() == 2 {
                    None
                } else {
                    Some(disconnected[0] as u8)
                },
                cause: 1,
            });
        }
        let due = now >= next.deadline_ms;
        let needs_bot = (0..2).any(|i| next.seats[i].bot.is_some() && next.pending[i].is_none());
        if next.ended.is_none() && !due && !needs_bot {
            return Ok(false);
        }
        let (new_state, new_events) = if next.ended.is_some() {
            next.rebuild()?
        } else {
            self.resolve(&mut next, due).await?
        };
        self.persist(next, &new_state).await?;
        *state = new_state;
        *events = new_events;
        Ok(true)
    }
    async fn resolve(
        &self,
        next: &mut Record,
        timeout: bool,
    ) -> Result<(engine::DuelState, EventBuf)> {
        let (state, _) = next.rebuild()?;
        for side in Side::BOTH {
            let i = side.index();
            if next.pending[i].is_none() {
                if let Some(bot) = next.seats[i].bot.clone() {
                    let mv = choose_bot(next, &state, side, bot).await?;
                    next.pending[i] = Some(protocol::duel::encode_move(&mv).encode_to_vec());
                    next.deadline_ms = next
                        .deadline_ms
                        .min(crate::online::now_ms() + self.settings.opponent_ms());
                } else if timeout {
                    next.timeouts[i] = next.timeouts[i].saturating_add(1);
                    next.pending[i] = Some(
                        protocol::duel::encode_move(&engine::default_move(&state, side))
                            .encode_to_vec(),
                    );
                }
            }
        }
        let timed: Vec<_> = (0..2)
            .filter(|i| next.timeouts[*i] >= engine_content::rules().duel.max_consecutive_timeouts)
            .collect();
        if !timed.is_empty() {
            next.ended = Some(ExternalEnd {
                loser: if timed.len() == 2 {
                    None
                } else {
                    Some(timed[0] as u8)
                },
                cause: 2,
            });
            return next.rebuild();
        }
        if let [Some(a), Some(b)] = &next.pending {
            next.moves.push([a.clone(), b.clone()]);
            next.pending = [None, None];
            next.deadline_ms = crate::online::now_ms() + self.settings.round_ms();
        }
        next.rebuild()
    }
}
async fn choose_bot(
    record: &Record,
    state: &engine::DuelState,
    side: Side,
    id: String,
) -> Result<engine::Move> {
    let fallback = engine::default_move(state, side);
    let expedition = record.kind == "expedition";
    let permit = if expedition {
        // Rewarded opponents must never become weaker merely because other AI slots are busy.
        tokio::time::timeout(Duration::from_millis(500), AI_SLOTS.acquire())
            .await
            .map_err(|_| Error("AI_BUSY"))?
            .map_err(|_| Error("AI_BUSY"))?
    } else {
        match AI_SLOTS.try_acquire() {
            Ok(permit) => permit,
            Err(_) => return Ok(fallback),
        }
    };
    let view = engine::view(state, side);
    let seed = u64::from_le_bytes(record.seed[..8].try_into().expect("seed"))
        ^ u64::from(state.round())
        ^ side.index() as u64;
    let history = (record.kind == "expedition").then(|| record.clone());
    let frozen = record.seats[side.index()].bot_profile.clone();
    let task = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let profile = frozen.or_else(|| {
            engine_ai::training_profiles()
                .bots
                .into_iter()
                .find(|p| p.id == id)
        });
        let profile = profile?;
        let mut bot = profile.bot(seed);
        if let Some(history) = history {
            let content = engine_content::content();
            if let Ok(setup) = history.setup()
                && let Ok(mut previous) = engine::new_duel(&setup, &content)
            {
                bot.observe(&engine::view(&previous, side));
                for pair in history.moves {
                    let [first, second] = pair.each_ref().map(|m| super::record::decode_move(m));
                    if let (Ok(first), Ok(second)) = (first, second) {
                        if engine::resolve_round(
                            &mut previous,
                            &content,
                            &[first, second],
                            &mut engine::EventBuf::new(),
                        )
                        .is_err()
                        {
                            break;
                        }
                        bot.observe(&engine::view(&previous, side));
                    }
                }
            }
        }
        bot.observe(&view);
        Some(bot.choose(&view, &engine_content::content()))
    });
    let timeout = if expedition {
        Duration::from_secs(2)
    } else {
        Duration::from_millis(50)
    };
    match tokio::time::timeout(timeout, task).await {
        Ok(Ok(Some(mv))) => Ok(mv),
        _ if expedition => Err(Error("AI_BUSY")),
        _ => Ok(fallback),
    }
}

pub fn snapshot(
    record: &Record,
    state: &engine::DuelState,
    events: &EventBuf,
    side: Side,
) -> Result<pb::DuelSnapshot> {
    let own = side.index();
    let other = side.other().index();
    let opponent = record.seats[other].public()?;
    Ok(pb::DuelSnapshot {
        duel_id: record.id.to_string(),
        kind: record.kind.clone(),
        you: Some(record.seats[own].public()?),
        opponent: Some(opponent.clone()),
        view: Some(protocol::duel::snapshot(state, side, opponent.template_id)),
        deadline_ms: record.deadline_ms,
        submitted: record.pending[own].is_some(),
        opponent_ready: record.pending[other].is_some(),
        opponent_connected: record.connected[other],
        grace_until_ms: record.grace_until[other].unwrap_or(0),
        seed_commitment: Sha256::digest(record.seed).to_vec(),
        events: events
            .as_slice()
            .iter()
            .filter_map(|e| protocol::duel::event_for(e, side))
            .collect(),
        state_hash: super::state_digest(state).to_vec(),
        revision: record.revision,
        your_move: record.pending[own]
            .as_ref()
            .map(|bytes| {
                pb::DuelMove::decode(bytes.as_slice()).map_err(|_| Error("INVALID_JOURNAL"))
            })
            .transpose()?,
        result: None,
    })
}

pub fn replay(record: &Record) -> Result<pb::MatchReplayData> {
    let mut rounds = Vec::with_capacity(record.moves.len() + 2);
    for count in 0..=record.moves.len() {
        let mut partial = record.clone();
        partial.moves.truncate(count);
        partial.pending = [None, None];
        partial.ended = None;
        let (state, events) = partial.rebuild()?;
        rounds.push(snapshot(&partial, &state, &events, Side::A)?);
    }
    if record.ended.is_some() {
        let (state, events) = record.rebuild()?;
        rounds.push(snapshot(record, &state, &events, Side::A)?);
    }
    Ok(pb::MatchReplayData {
        match_id: record.id.to_string(),
        rounds,
        seed: record.seed.to_vec(),
    })
}

#[cfg(test)]
mod ai_tests {
    use super::*;
    #[tokio::test]
    async fn rewarded_ai_waits_instead_of_using_a_weaker_default_when_slots_are_full() {
        let profile = engine_ai::training_profiles().bots[1].clone();
        let setup = profile.player(1, 0).unwrap();
        let public = pb::PublicPlayer {
            template_id: 1,
            level: 1,
            ..Default::default()
        };
        let seat = super::super::record::Seat::from_setup(
            None,
            None,
            &setup,
            &public,
            Some(profile.id.clone()),
        );
        let mut record = Record::new(
            Uuid::now_v7(),
            "expedition",
            [seat.clone(), seat],
            [0; 32],
            &Settings::default(),
        )
        .unwrap();
        let (state, _) = record.rebuild().unwrap();
        let _occupied = AI_SLOTS.acquire_many(4).await.unwrap();
        assert_eq!(
            choose_bot(&record, &state, Side::B, profile.id.clone())
                .await
                .err()
                .unwrap()
                .0,
            "AI_BUSY"
        );
        drop(_occupied);
        let mut archived = profile;
        archived.id = "retired-profile".into();
        record.seats[1].bot_profile = Some(archived);
        let mv = choose_bot(
            &record,
            &state,
            Side::B,
            "unknown-to-current-content".into(),
        )
        .await
        .unwrap();
        assert!(
            engine::view(&state, Side::B)
                .validate_move(&engine_content::content(), &mv)
                .is_ok()
        );
    }
}
