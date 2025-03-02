// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::js_hooks::{self, window};
use crate::net::{ReconnSocket, SocketUpdate, SystemInfo};
use crate::{
    dedup_into_inner, get_real_referrer, host, is_https, is_mobile, owned_into_box,
    owned_into_iter, post_message, timezone_offset, ws_protocol, AdEvent, Apply, ArenaQuery,
    BrowserStorages, ChatUpdate, ClaimValue, ClientActivity, ClientRequest, ClientUpdate,
    CommonRequest, CommonSettings, CommonUpdate, Escaping, GameClient, GameFence,
    InstancePickerDto, InvitationId, InvitationUpdate, KeyboardState, LeaderboardCaveat,
    LeaderboardScoreDto, LeaderboardUpdate, LiveboardDto, LiveboardUpdate, MessageDto,
    MessageNumber, MouseState, NavigationMetricsDto, NexusPath, PeriodId, PlayerDto, PlayerId,
    PlayerUpdate, QuestEvent, RankNumber, Referrer, SceneId, ScopeClaimKey, ServerId, SocketQuery,
    SystemUpdate, TeamId, VisibilityState, YourScoreDto,
};
use kodiak_common::arrayvec::ArrayString;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::LazyLock;
use wasm_bindgen::JsCast;
use web_sys::PerformanceNavigationTiming;
use yew::Callback;

#[cfg(feature = "audio")]
use crate::io::AudioPlayer;

/// The context (except rendering) of a game.
pub struct ClientContext<G: GameClient + ?Sized> {
    /// General client state.
    pub client: ClientState,
    /// Server state
    pub state: ServerState<G>,
    /// Server websocket
    pub socket:
        ReconnSocket<CommonUpdate<G::GameUpdate>, CommonRequest<G::GameRequest>, ServerState<G>>,
    /// Audio player (volume managed automatically).
    #[cfg(feature = "audio")]
    pub audio: AudioPlayer<G::Audio>,
    /// Keyboard input.
    pub keyboard: KeyboardState,
    /// Mouse input.
    pub mouse: MouseState,
    /// Whether the page is visible.
    pub visibility: VisibilityState,
    /// Settings.
    pub settings: G::GameSettings,
    /// Common settings.
    pub common_settings: CommonSettings,
    /// Local storage.
    pub browser_storages: BrowserStorages,
    pub(crate) referrer: Option<Referrer>,
    pub(crate) set_ui_props: Callback<G::UiProps>,
    pub send_ui_event: Callback<G::UiEvent>,
    pub(crate) system_info: Option<SystemInfo>,
    /// Time of last message sent to server (if it gets too high, need heartbeat).
    pub(crate) last_activity: f32,
    pub(crate) reported_activity: ClientActivity,
    socket_inbound: Callback<SocketUpdate<CommonUpdate<G::GameUpdate>>>,
}

/// State common to all clients.
#[derive(Default)]
pub struct ClientState {
    /// Time of last or current update.
    pub time_seconds: f32,
    /// Time of last keyboard/mouse input (for AFK purposes).
    pub(crate) last_input: f32,
    /// Supports rewarded ads.
    pub rewarded_ads: bool,
    /// Show escaping menu screen.
    pub escaping: Escaping,
    pub last_path: Option<NexusPath>,
}

/// Obtained from server via websocket.
pub struct ServerState<G: GameClient> {
    pub game: G::GameState,
    pub core: Rc<CoreState>,
    pub(crate) game_fence: Option<GameFence>,
    /// This state is from a closed websocket (i.e. because we are redirecting).
    ///
    /// First message received on new socket should reset state, including this flag.
    pub(crate) archived: bool,
}

impl<G: GameClient> Drop for ServerState<G> {
    fn drop(&mut self) {
        js_hooks::console_error!(
            "dropped the server state! panicking={}",
            std::thread::panicking()
        );
    }
}

/// Server state specific to core functions
#[derive(Default)]
pub struct CoreState {
    pub player_id: Option<PlayerId>,
    pub accepted_invitation_id: Option<InvitationId>,
    pub created_invitation_id: Option<InvitationId>,
    pub leaderboards: [Box<[LeaderboardScoreDto]>; std::mem::variant_count::<PeriodId>()],
    pub liveboard: Vec<LiveboardDto>,
    pub messages: VecDeque<(MessageNumber, MessageDto)>,
    pub players: HashMap<PlayerId, PlayerDto>,
    pub players_on_shard: u32,
    pub shard_per_scene: bool,
    pub players_online: u32,
    pub leaderboard_caveat: Option<LeaderboardCaveat>,
    pub temporaries_available: bool,
    pub servers: BTreeMap<(ServerId, SceneId), InstancePickerDto>,
    pub claims: HashMap<ScopeClaimKey, ClaimValue>,
    pub your_score: Option<YourScoreDto>,
}

impl<G: GameClient> Default for ServerState<G> {
    fn default() -> Self {
        Self {
            game: G::GameState::default(),
            core: Default::default(),
            game_fence: None,
            archived: false,
        }
    }
}

impl CoreState {
    pub fn rank(&self) -> Option<Option<RankNumber>> {
        self.claims
            .get(&ScopeClaimKey::rank())
            .map(|c| RankNumber::new(c.value.min(u8::MAX as u64) as u8))
    }

    /// Gets whether a player is friendly to an other player, taking into account team membership.
    /// Returns false if `other_player_id` is None.
    pub fn is_friendly(&self, other_player_id: Option<PlayerId>) -> bool {
        self.are_friendly(self.player_id, other_player_id)
    }

    /// Gets whether player is friendly to other player, taking into account team membership.
    /// Returns false if either `PlayerId` is None.
    pub fn are_friendly(
        &self,
        player_id: Option<PlayerId>,
        other_player_id: Option<PlayerId>,
    ) -> bool {
        player_id
            .zip(other_player_id)
            .map(|(id1, id2)| {
                id1 == id2
                    || self
                        .team_id_lookup(id1)
                        .zip(self.team_id_lookup(id2))
                        .map(|(id1, id2)| id1 == id2)
                        .unwrap_or(false)
            })
            .unwrap_or(false)
    }

    /// Gets player's `PlayerDto`.
    pub fn player(&self) -> Option<&PlayerDto> {
        self.player_id.and_then(|id| self.players.get(&id))
    }

    /// Player or bot (simulated) `PlayerDto`.
    pub fn player_or_bot(&self, player_id: PlayerId) -> Option<PlayerDto> {
        self.players.get(&player_id).cloned()
    }

    /// Gets player's `TeamId`.
    pub fn team_id(&self) -> Option<TeamId> {
        self.player_id.and_then(|id| self.team_id_lookup(id))
    }

    /// Gets a player's `TeamId`.
    fn team_id_lookup(&self, player_id: PlayerId) -> Option<TeamId> {
        self.players.get(&player_id).and_then(|p| p.team_id)
    }

    pub fn leaderboard(&self, period_id: PeriodId) -> &[LeaderboardScoreDto] {
        &self.leaderboards[period_id as usize]
    }
}

impl<G: GameClient> Apply<CommonUpdate<G::GameUpdate>> for ServerState<G> {
    fn apply(&mut self, update: CommonUpdate<G::GameUpdate>) {
        // Use rc_borrow_mut to keep semantics of shared references the same while sharing with
        // kodiak_client::yew.
        use rc_borrow_mut::RcBorrowMut;
        let mut core = Rc::borrow_mut(&mut self.core);

        match update {
            CommonUpdate::Chat(update) => {
                if let ChatUpdate::Received(received) = update {
                    let limit = if is_mobile() { 5 } else { 10 };
                    // Need to use into_vec since
                    // https://github.com/rust-lang/rust/issues/59878 is incomplete.
                    for (number, dto) in received.into_vec().into_iter() {
                        let dto = dedup_into_inner(dto);
                        if core.messages.len() >= limit {
                            core.messages.pop_front();
                        }
                        core.messages.push_back((number, dto));
                    }
                }
            }
            CommonUpdate::Client(update) => match update {
                ClientUpdate::SessionCreated { player_id, .. } => {
                    core.player_id = Some(player_id);
                }
                ClientUpdate::UpdateClaims(diff) => {
                    for (key, opt) in diff {
                        if let Some(value) = opt {
                            core.claims.insert(key, value);
                        } else {
                            core.claims.remove(&key);
                        }
                    }
                }
                ClientUpdate::ClearSyncState { game_fence } => {
                    self.game.reset();
                    self.game_fence = Some(game_fence);
                    core.liveboard.clear();
                    // Clearing leaderboard causes flicker with no discernable benefit (all servers are the same).
                    //core.leaderboards = Default::default();
                    core.players.clear();
                    core.messages.clear();
                    core.servers.clear();
                    core.claims.clear();
                    core.accepted_invitation_id = None;
                    core.your_score = Default::default();
                    core.leaderboard_caveat = Default::default();
                    // See leaderboard comment.
                    // core.players_online = 0;
                    core.players_on_shard = 0;
                }
                _ => {}
            },
            CommonUpdate::Game(update) => {
                self.game.apply(update);
            }
            CommonUpdate::Invitation(update) => match update {
                InvitationUpdate::Accepted(invitation_id) => {
                    core.accepted_invitation_id = invitation_id;
                }
                InvitationUpdate::Created(invitation_id) => {
                    core.created_invitation_id = Some(invitation_id);
                    post_message(&format!("createdInvitationId={invitation_id}"));
                }
            },
            CommonUpdate::Leaderboard(update) => match update {
                LeaderboardUpdate::Updated(period_id, leaderboard) => {
                    core.leaderboards[period_id as usize] = owned_into_box(leaderboard);
                }
            },
            CommonUpdate::Liveboard(LiveboardUpdate::Updated {
                liveboard,
                your_score,
                players_on_shard: players_on_server,
                shard_per_scene,
                players_online,
                caveat,
                temporaries_available,
            }) => {
                core.liveboard = owned_into_box(liveboard).into();
                core.your_score = your_score;
                core.players_on_shard = players_on_server;
                core.shard_per_scene = shard_per_scene;
                core.players_online = players_online;
                core.leaderboard_caveat = caveat;
                core.temporaries_available = temporaries_available;
            }
            CommonUpdate::Player(update) => {
                if let PlayerUpdate::Updated { added, removed } = update {
                    for player in owned_into_iter(added) {
                        core.players.insert(player.player_id, player);
                    }
                    for player_id in removed.iter() {
                        core.players.remove(player_id);
                    }
                }
            }
            CommonUpdate::System(update) => match update {
                SystemUpdate::Added(added) => {
                    for server in owned_into_iter(added) {
                        core.servers
                            .insert((server.server_id, server.scene_id), server);
                    }
                }
                SystemUpdate::Removed(removed) => {
                    for server_id in removed.iter() {
                        core.servers.remove(server_id);
                    }
                }
            },
        }
    }

    fn reset(&mut self) {
        let Self {
            archived,
            game,
            core,
            game_fence,
        } = self;
        std::mem::take(archived);
        std::mem::take(game_fence);
        game.reset();

        // Avoid dropping the `Rc` so there is less risk of broken weak references.
        use rc_borrow_mut::RcBorrowMut;
        *<Rc<_> as RcBorrowMut<CoreState>>::borrow_mut(core) = Default::default();
    }
}

impl<G: GameClient> ClientContext<G> {
    pub(crate) fn new(
        mut browser_storages: BrowserStorages,
        mut common_settings: CommonSettings,
        settings: G::GameSettings,
        socket_inbound: Callback<SocketUpdate<CommonUpdate<G::GameUpdate>>>,
        set_ui_props: Callback<G::UiProps>,
        send_ui_event: Callback<G::UiEvent>,
        system_info: Option<SystemInfo>,
    ) -> Self {
        common_settings.set_server_id(
            system_info.as_ref().map(|i| i.server_id),
            &mut browser_storages,
        );
        // Don't set arena id here, trust that the ideal server will give us *an* arena.
        let referrer = get_real_referrer(G::GAME_CONSTANTS.domain);
        let host = Self::compute_websocket_host(&common_settings, &system_info, referrer);
        let socket = ReconnSocket::new(host, G::GAME_CONSTANTS.udp_enabled, socket_inbound.clone());

        Self {
            #[cfg(feature = "audio")]
            audio: AudioPlayer::default(),
            client: ClientState::default(),
            state: ServerState::default(),
            socket,
            keyboard: KeyboardState::default(),
            mouse: MouseState::default(),
            visibility: VisibilityState::default(),
            settings,
            common_settings,
            browser_storages,
            set_ui_props,
            send_ui_event,
            system_info,
            referrer,
            last_activity: 0.0,
            reported_activity: ClientActivity::default(),
            socket_inbound,
        }
    }

    pub(crate) fn cancel_afk(&mut self) {
        self.client.last_input = self.client.time_seconds;
    }

    pub(crate) fn client_activity(&self) -> ClientActivity {
        if self.visibility.is_hidden() {
            ClientActivity::Hidden
        } else if self.client.last_input < self.client.time_seconds - 10.0 {
            ClientActivity::Afk
        } else {
            ClientActivity::Active
        }
    }

    pub(crate) fn heartbeat(&mut self) {
        let client_activity = self.client_activity();
        self.send_to_server(CommonRequest::Client(ClientRequest::Heartbeat(
            client_activity,
        )));
        self.reported_activity = client_activity;
    }

    /// [`None`] means default/initial/current).
    pub fn choose_server_id(&mut self, server_id: Option<ServerId>, arena_id: ArenaQuery) {
        /*
        let server_id_is_default =
            self.common_settings.server_id == self.system_info.as_ref().map(|s| s.ideal_server_id);
        let server_id_matches = (server_id.is_none() && server_id_is_default)
            || server_id == self.common_settings.server_id;
        let arena_id_matches = arena_id == self.common_settings.arena_id;
        //let invitation_id_matches = invitation_id == self.common_settings.invitation_id;
        if server_id_matches && arena_id_matches && invitation_id.is_some() {
            return;
        }
        */
        js_hooks::console_log!(
            "choosing {:?} over {:?}",
            (server_id, arena_id),
            (
                self.common_settings.server_id,
                self.common_settings.arena_id,
            )
        );
        // Invalidate old server state.
        self.state.archived = true;

        self.common_settings
            .set_server_id(server_id, &mut self.browser_storages);
        self.common_settings
            .set_arena_id(arena_id, &mut self.browser_storages);
        let host =
            Self::compute_websocket_host(&self.common_settings, &self.system_info, self.referrer);
        let (old_url, _) = self
            .socket
            .host()
            .split_once('?')
            .unwrap_or((self.socket.host(), ""));
        let (url, query) = host.split_once('?').unwrap_or((&host, ""));
        if old_url == url {
            self.socket.reset_host(host.clone());
            self.socket.send(
                CommonRequest::Redial {
                    query_string: query.into(),
                },
                true,
            );
        } else {
            self.socket = ReconnSocket::new(
                host,
                G::GAME_CONSTANTS.udp_enabled,
                self.socket_inbound.clone(),
            );
        }
    }

    pub(crate) fn compute_websocket_host(
        common_settings: &CommonSettings,
        system_info: &Option<SystemInfo>,
        referrer: Option<Referrer>,
    ) -> String {
        static NAVIGATION_METRICS: LazyLock<NavigationMetricsDto> = LazyLock::new(|| {
            let mut ret = NavigationMetricsDto::default();

            if let Some(performance) = window().performance()
                && let Ok(navigation) = performance
                    .get_entries_by_type("navigation")
                    .get(0)
                    .dyn_into::<PerformanceNavigationTiming>()
            {
                ret.dns = (navigation.domain_lookup_end() - navigation.domain_lookup_start()).ceil()
                    as u16;
                let secure_connection_start = navigation.secure_connection_start();
                let mut connect_end = secure_connection_start;
                if connect_end == 0.0 {
                    connect_end = navigation.connect_end();
                }
                ret.tcp = (connect_end - navigation.connect_start()).ceil() as u16;
                if secure_connection_start > 0.0 {
                    ret.tls = (navigation.connect_end() - secure_connection_start).ceil() as u16;
                }
                ret.http = (navigation.response_end() - navigation.request_start()).ceil() as u16;
                ret.dom =
                    (navigation.load_event_end() - navigation.dom_interactive()).ceil() as u16;
            }

            ret
        });

        let (encryption, host) = common_settings
            .server_id
            //.filter(|_| !host.starts_with("localhost"))
            .map(|id: ServerId| {
                if id.kind.is_cloud() {
                    let hostname = if let Some(alias) = system_info.as_ref().and_then(|s| {
                        let current = host();
                        s.alternative_domains
                            .iter()
                            .find(|a| current.ends_with(&***a))
                    }) {
                        format!("{}_{}", id.number, alias)
                    } else {
                        G::GAME_CONSTANTS.hostname(id)
                    };
                    (true, hostname)
                } else {
                    let host = system_info
                        .as_ref()
                        .map(|i| i.host.to_owned())
                        .unwrap_or_else(host);
                    let name = host.split_once(':').map(|(host, _)| host).unwrap_or(&host);
                    let port = host
                        .split_once(':')
                        .and_then(|(_, port)| port.parse::<u16>().ok())
                        .filter(|port| *port <= 8080 || *port == 8443)
                        .unwrap_or(8000 + id.number.0.get() as u16);
                    (
                        system_info
                            .as_ref()
                            .map(|i| i.encryption)
                            .unwrap_or_else(is_https),
                        format!("{name}:{port}"),
                    )
                }
            })
            .unwrap_or_else(|| {
                (
                    system_info
                        .as_ref()
                        .map(|i| i.encryption)
                        .unwrap_or_else(is_https),
                    system_info
                        .as_ref()
                        .map(|i| i.host.clone())
                        .unwrap_or_else(host),
                )
            });

        // crate::console_log!("override={:?} ideal server={:?}, host={:?}, ideal_host={:?}", override_server_id, ideal_server_id, host, ideal_host);

        let web_socket_query = SocketQuery {
            arena_id: common_settings.arena_id,
            session_token: common_settings.session_token,
            date_created: common_settings.date_created,
            cohort_id: common_settings.cohort_id,
            language_id: common_settings.language,
            referrer,
            timezone_offset: timezone_offset(),
            user_agent: window()
                .navigator()
                .user_agent()
                .ok()
                .and_then(|ua| ArrayString::from(&ua).ok()),
            dns: NAVIGATION_METRICS.dns,
            tcp: NAVIGATION_METRICS.tcp,
            tls: NAVIGATION_METRICS.tls,
            http: NAVIGATION_METRICS.http,
            dom: NAVIGATION_METRICS.dom,
        };

        // TODO to_string should take &impl Serialize.
        let web_socket_query_url = serde_urlencoded::to_string(web_socket_query).unwrap();

        format!(
            "{}://{}/ws?{}",
            ws_protocol(encryption),
            host,
            web_socket_query_url
        )
    }

    /// Shorter version of `context.state.core.player_id`.
    pub fn player_id(&self) -> Option<PlayerId> {
        self.state.core.player_id
    }

    /// Whether the game websocket is closed or errored (not open, opening, or nonexistent).
    pub fn connection_lost(&self) -> bool {
        self.socket.is_terminated()
    }

    /// Send a game command on the socket.
    pub fn send_to_game(&mut self, request: G::GameRequest) {
        self.send_to_game_with_reliable(request, true);
    }

    /// Optionally faster but less reliable than `send_to_game`. Unreliable messages are
    /// dropped if the websocket is still opening.
    pub fn send_to_game_with_reliable(&mut self, request: G::GameRequest, reliable: bool) {
        self.send_to_server_with_reliable(
            CommonRequest::Game(request, self.state.game_fence),
            reliable,
        );
    }

    /// Whether can send unreliable messages.
    pub fn supports_unreliable(&self) -> bool {
        self.socket.supports_unreliable()
    }

    /// Send a request to log an error message.
    pub fn send_trace(&mut self, mut message: String) {
        message.truncate(message.floor_char_boundary(QuestEvent::TRACE_LIMIT));
        self.send_to_server(CommonRequest::Client(ClientRequest::RecordQuestEvent(
            QuestEvent::Trace {
                message: message.into(),
            },
        )));
    }

    /// Send a request on the socket.
    pub fn send_to_server(&mut self, request: CommonRequest<G::GameRequest>) {
        self.send_to_server_with_reliable(request, true)
    }

    fn send_to_server_with_reliable(
        &mut self,
        request: CommonRequest<G::GameRequest>,
        reliable: bool,
    ) {
        self.last_activity = self.client.time_seconds;
        self.socket.send(request, reliable);
    }

    /// Set the props used to render the UI.
    pub fn set_ui_props(&mut self, props: G::UiProps, in_game: bool) {
        if in_game && self.client.escaping.is_spawning() {
            #[cfg(not(feature = "pointer_lock"))]
            let in_game = true;
            #[cfg(feature = "pointer_lock")]
            let in_game = self.mouse.pointer_locked;
            self.set_escaping(if in_game {
                Escaping::InGame
            } else {
                Escaping::Escaping
            });
        }
        if !in_game && !self.client.escaping.is_spawning() {
            self.set_escaping(Escaping::Spawning);
        }
        self.set_ui_props.emit(props);
    }

    pub fn set_escaping(&mut self, escaping: Escaping) {
        if escaping.is_in_game() {
            #[cfg(feature = "pointer_lock")]
            if !self.mouse.pointer_locked {
                crate::request_pointer_lock_with_emulation();
                // The pointer-lock change will result in InGame.
                return;
            }
        } else {
            self.keyboard.reset();
            // Will exit pointer lock.
            self.mouse.reset();
        }

        if escaping != self.client.escaping {
            escaping.post_message();
        } else {
            js_hooks::console_log!("repeated escaping {escaping:?}");
        }
        self.client.escaping = escaping;
    }

    /// Enable visibility cheating.
    pub fn cheats(&self) -> bool {
        cfg!(debug_assertions) || self.state.core.player().map(|p| p.admin).unwrap_or(false)
    }

    pub fn enable_rewarded_ads(&mut self) {
        self.client.rewarded_ads = true;
    }

    /// Sends any request to the server.
    pub fn send_request(&mut self, request: CommonRequest<G::GameRequest>) {
        self.send_to_server(request);
    }

    /// Call when an advertisement was played.
    pub fn ad_event(&mut self, event: AdEvent) {
        self.send_to_server(CommonRequest::Client(ClientRequest::TallyAd(event)));
    }

    /// Simulates dropping of one or both websockets.
    pub fn simulate_drop_socket(&mut self) {
        self.socket.simulate_drop();
    }
}

#[derive(Clone)]
pub struct WeakCoreState(std::rc::Weak<CoreState>);

impl Default for WeakCoreState {
    fn default() -> Self {
        thread_local! {
            static DEFAULT_CORE_STATE: Rc<CoreState> = Rc::default();
        }
        DEFAULT_CORE_STATE.with(Self::new) // Only allocate zero value once to not cause a leak.
    }
}

impl PartialEq for WeakCoreState {
    fn eq(&self, _other: &Self) -> bool {
        false // Can't implement Eq because not reflexive but probably doesn't matter...
    }
}

impl WeakCoreState {
    /// Borrow the core state immutably. Unused for now.
    pub fn as_strong(&self) -> StrongCoreState {
        StrongCoreState {
            inner: self.0.upgrade().unwrap(),
            _spooky: PhantomData,
        }
    }

    /// Like [`Self::as_strong`] but consumes self and has a static lifetime.
    pub fn into_strong(self) -> StrongCoreState<'static> {
        StrongCoreState {
            inner: self.0.upgrade().unwrap(),
            _spooky: PhantomData,
        }
    }

    /// Create a [`WeakCoreState`] from a [`Rc<CoreState>`].
    pub fn new(core: &Rc<CoreState>) -> Self {
        Self(Rc::downgrade(core))
    }
}

pub struct StrongCoreState<'a> {
    inner: Rc<CoreState>,
    _spooky: PhantomData<&'a ()>,
}

impl<'a> std::ops::Deref for StrongCoreState<'a> {
    type Target = CoreState;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
