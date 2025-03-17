// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::{Arena, ChatInbox};
use crate::actor::PlasmaActlet;
use crate::bitcode::{self, *};
use crate::service::{ArenaService, BotRepo, MetricRepo, Player, PlayerRepo};
use crate::{
    ArenaId, ArenaSettingsDto, ChatId, ChatMessage, ChatRecipient, ChatRequest, ChatUpdate,
    MessageDto, MessageNumber, NonZeroUnixMillis, PlasmaRequestV1, PlayerAlias, PlayerId,
    QuestEvent, RealmId, SceneId, ServerNumber, UnixTime,
};
use kodiak_common::arrayvec::ArrayString;
use kodiak_common::heapless::HistoryBuffer;
use kodiak_common::slice_up_to_array_string;
use std::collections::HashSet;
use std::marker::PhantomData;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

/// Component of [`Context`] dedicated to chat.
pub struct ChatRepo<G> {
    /// For new players' chat to start full.
    recent: HistoryBuffer<(Arc<MessageDto>, Option<MessageAttribution>), 16>,
    /// For uniqueness.
    last_timestamp: NonZeroUnixMillis,
    _spooky: PhantomData<G>,
}

#[derive(Copy, Clone, Debug, Encode, Decode)]
pub struct MessageAttribution {
    pub(crate) chat_id: ChatId,
    pub(crate) sender_ip: IpAddr,
}

/// Component of client data encompassing chat information.
#[derive(Clone, Debug, Default, Encode, Decode)]
pub struct ClientChatData {
    /// Players this client has muted.
    muted: HashSet<IpAddr>,
    /// Messages that have/will be sent to the client.
    pub(crate) inbox: ChatInbox,
    /// `None` if not yet announced. Cleared if traveling between different realms.
    pub(crate) join_announced: Option<SceneId>,
}

impl ClientChatData {
    /// Receives a message (unless the sender is muted).
    ///
    /// `foreign_sender_ip` should be `None` if sending to self.
    pub fn receive(&mut self, message: &Arc<MessageDto>, attribution: Option<MessageAttribution>) {
        if attribution
            .map(|attribution: MessageAttribution| self.muted.contains(&attribution.sender_ip))
            .unwrap_or(false)
        {
            // Muted.
            return;
        }
        self.inbox.write(Arc::clone(message), attribution);
    }
}

impl<G: ArenaService> Default for ChatRepo<G> {
    fn default() -> Self {
        Self {
            recent: HistoryBuffer::new(),
            last_timestamp: NonZeroUnixMillis::MIN,
            _spooky: PhantomData,
        }
    }
}

impl<G: ArenaService> ChatRepo<G> {
    /// Indicate a preference to not receive further messages from a given player.
    fn mute_player(
        &mut self,
        req_player_id: PlayerId,
        mute_message_number: MessageNumber,
        players: &mut PlayerRepo<G>,
    ) -> Result<ChatUpdate, &'static str> {
        let mute_ip = Self::sender_attribution(req_player_id, mute_message_number, &*players)
            .map(|a| a.sender_ip)
            .ok_or("unknown sender")?;
        let req_player = players.get_mut(req_player_id).ok_or("nonsense")?;
        let req_client = req_player.client_mut().ok_or("only clients can mute")?;

        if req_client.chat.muted.len() >= 32 {
            return Err("too many muted");
        }

        if req_client.chat.muted.insert(mute_ip) {
            Ok(ChatUpdate::Muted(mute_message_number))
        } else {
            Err("already muted")
        }
    }

    /// Indicate a preference to receive further messages from a given player.
    fn unmute_player(
        &mut self,
        req_player_id: PlayerId,
        unmute_message_number: MessageNumber,
        players: &mut PlayerRepo<G>,
    ) -> Result<ChatUpdate, &'static str> {
        let unmute_ip = Self::sender_attribution(req_player_id, unmute_message_number, &*players)
            .map(|a| a.sender_ip)
            .ok_or("unknown sender")?;
        let req_player = players.get_mut(req_player_id).ok_or("nonsense")?;
        let req_client = req_player.client_mut().ok_or("only clients can unmute")?;

        req_client.chat.muted.remove(&unmute_ip);

        // Don't error if wasn't muted to avoid leaking IP correspondences between players.
        Ok(ChatUpdate::Unmuted(unmute_message_number))
    }

    fn sender_attribution(
        req_player_id: PlayerId,
        message_number: MessageNumber,
        players: &PlayerRepo<G>,
    ) -> Option<MessageAttribution> {
        let req_player = players.get(req_player_id)?;
        let req_client = req_player.client()?;
        req_client.chat.inbox.attribute(message_number)
    }

    fn report(
        &mut self,
        req_player_id: PlayerId,
        restrict_message_number: MessageNumber,
        players: &mut PlayerRepo<G>,
        metrics: &mut MetricRepo<G>,
        plasma: &PlasmaActlet,
    ) -> Result<ChatUpdate, &'static str> {
        let report_attribution =
            Self::sender_attribution(req_player_id, restrict_message_number, &*players)
                .ok_or("unknown sender")?;
        let req_player = players.get_mut(req_player_id).ok_or("nonsense")?;
        let alive_duration = req_player
            .alive_duration()
            .map(|d| d < Duration::from_secs(60))
            .unwrap_or(true);
        let req_client = req_player.inner.client_mut().ok_or("not a real player")?;
        // Admins can override for testing.
        if req_client.ip_address == report_attribution.sender_ip
            && !(req_client.admin() && cfg!(debug_assertions))
        {
            return Err("cannot restrict own IP");
        }
        if !req_client.moderator() {
            if alive_duration {
                return Err("report requirements unmet");
            }
            if req_client.reported.len() >= 6 {
                return Err("too many reports");
            }
            if !req_client.reported.insert(report_attribution.sender_ip) {
                return Err("already reported");
            }
        }
        metrics.mutate_with(|m| m.abuse_reports.increment(), &req_client.metrics);
        if let Some(visitor_id) = req_client.session.visitor_id {
            plasma.do_request(PlasmaRequestV1::ModerateAbuse {
                alias: req_player.alias,
                chat_id: report_attribution.chat_id,
                visitor_id,
            });
        }
        Ok(ChatUpdate::PlayerRestricted {
            message_number: restrict_message_number,
        })
    }

    fn set_safe_mode(
        &mut self,
        minutes: u32,
        req_player: &Player<G>,
        realm_id: RealmId,
        plasma: &PlasmaActlet,
    ) -> Result<ChatUpdate, &'static str> {
        let req_client = req_player.client().ok_or("not a real player")?;
        if !req_client.moderator() {
            return Err("permission denied");
        }
        if let Some(visitor_id) = req_client.session.visitor_id {
            plasma.do_request(PlasmaRequestV1::ModerateChat {
                alias: req_player.alias,
                // TODO: ideally, the caller should pass in the arena ID instead of the realm_id.
                arena_id: ArenaId::new(realm_id, SceneId::default()),
                visitor_id,
                safe_mode: Some(minutes),
                slow_mode: None,
            });
        }
        Ok(ChatUpdate::SafeModeSet(minutes))
    }

    fn set_slow_mode(
        &mut self,
        minutes: u32,
        req_player: &Player<G>,
        realm_id: RealmId,
        plasma: &PlasmaActlet,
    ) -> Result<ChatUpdate, &'static str> {
        let req_client = req_player.client().ok_or("not a real player")?;
        if !req_client.moderator() {
            return Err("permission denied");
        }
        if let Some(visitor_id) = req_client.session.visitor_id {
            plasma.do_request(PlasmaRequestV1::ModerateChat {
                alias: req_player.alias,
                // TODO: ideally, the caller should pass in the arena ID instead of the realm_id.
                arena_id: ArenaId::new(realm_id, SceneId::default()),
                visitor_id,
                safe_mode: None,
                slow_mode: Some(minutes),
            });
        }
        Ok(ChatUpdate::SlowModeSet(minutes))
    }

    /// Send a chat to all players, or one's team (whisper).
    #[allow(clippy::too_many_arguments)]
    fn send_chat(
        &mut self,
        req_arena_id: ArenaId,
        req_player_id: PlayerId,
        message: String,
        whisper: bool,
        req_tier: &mut Arena<G>,
        metrics: &mut MetricRepo<G>,
        plasma: &PlasmaActlet,
    ) -> Result<ChatUpdate, &'static str> {
        let req_player = req_tier
            .arena_context
            .players
            .get_mut(req_player_id)
            .ok_or("nonexistent player")?;
        if !req_player.regulator.active() {
            return Err("inactive");
        }
        if let Some(req_client) = req_player.client_mut() {
            req_client.push_quest(QuestEvent::Chat { whisper });
            metrics.mutate_with(
                |metrics| {
                    metrics.chats.increment();
                },
                &req_client.metrics,
            );
        }
        let whisper = whisper || req_tier.arena_service.force_whisper(req_player_id);
        let team_name = req_tier.arena_service.get_team_name(req_player_id);

        if let Some(text) = self.try_execute_command(
            req_arena_id.realm_id,
            req_player_id,
            &message,
            &mut req_tier.arena_service,
            req_player,
            &req_tier.arena_context.bots,
            &mut req_tier.arena_context.settings,
            plasma,
        ) {
            let alias = req_player.alias;
            if let Some(req_client) = req_player.inner.client_mut() {
                let timestamp = NonZeroUnixMillis::now().max(self.last_timestamp.add_millis(1));
                self.last_timestamp = timestamp;
                plasma.do_request(PlasmaRequestV1::SendChat {
                    arena_id: req_arena_id,
                    timestamp,
                    team_name,
                    alias,
                    visitor_id: req_client.session.visitor_id,
                    player_id: Some(req_player_id),
                    admin: false,
                    authentic: req_client
                        .nick_name()
                        .map(|n| n.as_str() == req_player.alias.as_str())
                        .unwrap_or(false),
                    recipient: ChatRecipient::None,
                    message,
                    ip_address: req_client.ip_address,
                });

                let message = MessageDto {
                    alias: PlayerAlias::authority(),
                    visitor_id: None,
                    team_name: None,
                    authority: true,
                    authentic: true,
                    message: ChatMessage::Raw {
                        message: text,
                        detected_language_id: Default::default(),
                        english_translation: None,
                    },
                    whisper,
                };
                req_client.chat.receive(&Arc::new(message), None);
            } else {
                debug_assert!(false, "bot issued command");
            }
            return Ok(ChatUpdate::Sent);
        }

        let team = req_tier.arena_service.get_team_members(req_player_id);

        /*
        if !req_player.was_ever_alive {
            return Err("must be in game to chat");
        }
        */

        if whisper && team.is_none() {
            return Err("no one to whisper to");
        }

        // If the team no longer exists, no members should exist.
        debug_assert_eq!(req_player.team_id.is_some(), team.is_some());

        let Some(req_client) = req_player.inner.client_mut() else {
            return Err("not a client");
        };

        const COMPLAINT_MESSAGES: &'static [&'static str] = &["lag"];
        const COMPLAINT_PHRASES: &'static [&'static str] = &[
            "game crashed",
            "game froze",
            "game broke",
            "game freezing",
            "hacks",
            "hacker",
            "hacking",
            "hacked",
            "cheats",
            "cheater",
            "cheating",
            "cheated",
            "i hate this game",
            "this game sucks",
            "game is bad",
            "game is awful",
            "game is pretty awful",
            "stupid game",
            "bad game",
            "dumb game",
            "worst game",
            "terrible game",
            "laggy",
            "lagged",
            "lagging",
            "not fun",
            "game isnt fun",
            "game isn't fun",
            "game is not fun",
            "fkin game",
            "dont like this game",
            "don't like this game",
            "dont like the game",
            "don't like the game",
            "lost connection",
            "network error",
            "network issue",
            "high latency",
            "low fps",
            //"fake",
        ];

        if !req_client.metrics.complained {
            let mut buffer: ArrayString<50> = slice_up_to_array_string(&message);
            buffer.make_ascii_lowercase();
            for &complaint_phrase in COMPLAINT_PHRASES {
                if buffer.contains(complaint_phrase) {
                    req_client.metrics.complained = true;
                    break;
                }
            }
            for &complaint_message in COMPLAINT_MESSAGES {
                if &buffer == complaint_message {
                    req_client.metrics.complained = true;
                    break;
                }
            }
        }

        let authentic = req_client
            .nick_name()
            .map(|n| n.as_str() == req_player.alias.as_str())
            .unwrap_or(false);

        let timestamp = NonZeroUnixMillis::now().max(self.last_timestamp.add_millis(1));
        self.last_timestamp = timestamp;
        plasma.do_request(PlasmaRequestV1::SendChat {
            admin: false,
            alias: req_player.alias,
            authentic,
            ip_address: req_client.ip_address,
            message,
            arena_id: req_arena_id,
            team_name,
            player_id: Some(req_player_id),
            timestamp,
            visitor_id: req_client.session.visitor_id,
            recipient: if whisper {
                ChatRecipient::TeamOf(req_player_id)
            } else {
                ChatRecipient::Broadcast
            },
        });
        Ok(ChatUpdate::Sent)
    }

    /// Broadcasts a message to all players (including queuing it for those who haven't joined yet).
    pub(crate) fn broadcast_message<'a>(
        &mut self,
        message: Arc<MessageDto>,
        attribution: Option<MessageAttribution>,
        tiers: impl IntoIterator<Item = &'a mut Arena<G>>,
        sender_player_id: Option<PlayerId>,
        save_recent: bool,
    ) {
        for tier in tiers {
            for (player_id, player) in tier.arena_context.players.iter_mut() {
                if let Some(client) = player.client_mut() {
                    client.chat.receive(
                        &message,
                        attribution.filter(|_| sender_player_id != Some(player_id)),
                    );
                }
            }
        }
        if save_recent {
            self.recent.write((message, attribution));
        }
    }

    /// Process any [`ChatRequest`].
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_chat_request(
        &mut self,
        req_arena_id: ArenaId,
        req_player_id: PlayerId,
        request: ChatRequest,
        req_tier: &mut Arena<G>,
        metrics: &mut MetricRepo<G>,
        plasma: &PlasmaActlet,
    ) -> Result<ChatUpdate, &'static str> {
        let context = &mut req_tier.arena_context;
        match request {
            ChatRequest::Mute(message_number) => {
                self.mute_player(req_player_id, message_number, &mut context.players)
            }
            ChatRequest::Unmute(message_number) => {
                self.unmute_player(req_player_id, message_number, &mut context.players)
            }
            ChatRequest::Send { message, whisper } => self.send_chat(
                req_arena_id,
                req_player_id,
                message,
                whisper,
                req_tier,
                metrics,
                plasma,
            ),
            ChatRequest::SetSafeMode(minutes) => {
                let req_player = context
                    .players
                    .get_mut(req_player_id)
                    .ok_or("nonexistent player")?;
                self.set_safe_mode(minutes, req_player, req_arena_id.realm_id, plasma)
            }
            ChatRequest::SetSlowMode(minutes) => {
                let req_player = context
                    .players
                    .get_mut(req_player_id)
                    .ok_or("nonexistent player")?;
                self.set_slow_mode(minutes, req_player, req_arena_id.realm_id, plasma)
            }
            ChatRequest::Report(message_number) => self.report(
                req_player_id,
                message_number,
                &mut context.players,
                metrics,
                plasma,
            ),
        }
    }

    /// Back-fills player with recent messages.
    pub fn initialize_client(
        &self,
        server_number: ServerNumber,
        arena_id: ArenaId,
    ) -> ClientChatData {
        let mut chat = ClientChatData::default();
        for (msg, attribution) in self.recent.oldest_ordered() {
            chat.receive(msg, *attribution);
        }
        let intro = Arc::new(MessageDto {
            alias: PlayerAlias::authority(),
            visitor_id: None,
            team_name: None,
            message: ChatMessage::Welcome {
                server_number,
                arena_id,
            },
            authority: true,
            authentic: false,
            whisper: false,
        });
        chat.receive(&intro, None);
        chat
    }

    /// Gets chat update, consisting of new messages, for a player.
    pub fn player_delta(chat: &mut ClientChatData) -> Option<ChatUpdate> {
        if chat.inbox.read_is_empty() {
            None
        } else {
            Some(ChatUpdate::Received(chat.inbox.read()))
        }
    }

    fn try_execute_command(
        &mut self,
        req_realm_id: RealmId,
        req_player_id: PlayerId,
        message: &str,
        service: &mut G,
        player: &mut Player<G>,
        bots: &BotRepo<G>,
        settings: &mut ArenaSettingsDto<G::ArenaSettings>,
        plasma: &PlasmaActlet,
    ) -> Option<String> {
        fn parse_minutes(arg: &str) -> Option<u32> {
            if matches!(arg, "none" | "off") {
                Some(0)
            } else {
                arg.parse::<u32>()
                    .ok()
                    .or_else(|| arg.strip_suffix('m').and_then(|s| s.parse().ok()))
                    .or_else(|| {
                        arg.strip_suffix('h')
                            .and_then(|s| s.parse::<u32>().ok())
                            .and_then(|n| n.checked_mul(60))
                    })
            }
        }

        let command = message.strip_prefix('/')?;
        let mut words = command.split_ascii_whitespace();
        let first = words.next()?;

        macro_rules! until {
            ($name: literal, $setter: ident) => {{
                match words.next() {
                    None => String::from("missing number of minutes"),
                    Some(arg) => {
                        if let Some(minutes) = parse_minutes(arg) {
                            self.$setter(minutes, player, req_realm_id, plasma)
                                .map(|_| "done".to_owned())
                                .unwrap_or_else(|e| String::from(e))
                        } else {
                            String::from("failed to parse argument as minutes")
                        }
                    }
                }
            }};
        }

        Some(match first {
            "slow" => until!("slow mode", set_slow_mode),
            "safe" => until!("safe mode", set_safe_mode),
            "bots" => {
                if let Some(client) = player.client()
                    && (cfg!(debug_assertions) || client.admin())
                {
                    let hard_max = if cfg!(debug_assertions) { 64 } else { 1024 };
                    if let Some(count) = words.next() {
                        if let Some(count) = count.parse::<u16>().ok()
                            && count <= hard_max
                        {
                            settings.engine.bots = Some(count);
                            "OK".to_owned()
                        } else if count == "default" {
                            settings.engine.bots = None;
                            "OK".to_owned()
                        } else {
                            "error".to_owned()
                        }
                    } else {
                        bots.count.to_string()
                    }
                } else {
                    "permission denied".to_owned()
                }
            }
            "bot_aggression" => {
                if let Some(client) = player.client()
                    && (cfg!(debug_assertions) || client.admin())
                {
                    if let Some(aggression) = words.next() {
                        if let Some(aggression) = aggression.parse::<f32>().ok()
                            && (0.0..=10.0).contains(&aggression)
                        {
                            settings.engine.bot_aggression = Some(aggression);
                            "OK".to_owned()
                        } else if aggression == "default" {
                            settings.engine.bot_aggression = None;
                            "OK".to_owned()
                        } else {
                            "error".to_owned()
                        }
                    } else {
                        settings.engine.bot_aggression().to_string()
                    }
                } else {
                    "permission denied".to_owned()
                }
            }
            _ => service
                .chat_command(command, req_player_id, player)
                .unwrap_or_else(|| String::from("unrecognized command")),
        })
    }
}
