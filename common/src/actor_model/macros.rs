// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

/// Helper macro for common singleton pattern, extracting the `Singleton` from the `World`.
#[macro_export]
macro_rules! singleton {
    ($world:expr) => {
        match $world.singleton.as_ref() {
            None => {
                debug_assert!(
                    $world.is_default(),
                    "missing singleton but has other actors"
                );
                None
            }
            Some(s) => Some(&s.1.actor),
        }
    };
}

/// Same as [`singleton`] but mutable.
/// ```ignore
/// // E.g. at the start of World tick:
/// let Some(mut singleton) = singleton_mut!(self) else {
///     return;
/// };
/// ```
#[macro_export]
macro_rules! singleton_mut {
    ($world:ident) => {
        match $world.singleton.as_mut() {
            None => {
                debug_assert!(
                    $world.is_default(),
                    "missing singleton but has other actors"
                );
                None
            }
            Some(s) => Some(&mut s.1.actor),
        }
    };
}

/// Shorthand for applying events.
#[macro_export]
macro_rules! apply {
    ($me:ident, $actor:ident, $src:ident, $event:ident, $context:expr) => {
        $crate::actor_model::paste! {
            for (dst, state) in $crate::actor_model::Map::iter_mut(&mut $me.[<$actor:snake>]) {
                let mut context = $crate::actor_model::Dst::new(&mut *$context, dst);
                for (src, events) in $crate::actor_model::Map::iter(&state.inbox.[<$src:snake>]) {
                    let context = &mut $crate::actor_model::Src::new(&mut context, src);
                    state.actor.apply(&events.[<$event:snake>], context);
                }
            }
        }
    };
}

/// Shorthand for applying events from [`Server`] aka inputs in [`WorldTick::tick_client`].
#[macro_export]
macro_rules! apply_inputs {
    ($me:ident, $actor:ident, $input:ident, $context:expr) => {
        apply!($me, $actor, Server, $input, $context);
    };
}

// Define Apply traits at call site to fix:
// type parameter `T` must be covered by another type when it appears before the first local type (`ActorEventsFromActorId`)
#[doc(hidden)]
#[macro_export]
macro_rules! define_apply {
    () => {
        /// A type that can be mutated by a `&U`. Also takes a context for callbacks.
        pub trait Apply<U, C: ?Sized> {
            fn apply(&mut self, u: &U, context: &mut C);
        }

        /// Like [`Apply`] but takes an owned `U`. TODO find a way to have 1 Apply trait.
        pub trait ApplyOwned<U, C: ?Sized> {
            fn apply_owned(&mut self, u: U, context: &mut C);
        }

        // Allows array based types other than Vec.
        impl<T: Apply<U, C>, U, C: ?Sized, D: std::ops::Deref<Target = [U]>> Apply<D, C> for T {
            fn apply(&mut self, d: &D, context: &mut C) {
                let slice: &[U] = &*d;
                for u in slice {
                    self.apply(u, context);
                }
            }
        }
    };
}

#[macro_export]
macro_rules! define_events {
    ($actor:ident, $src:ident $(, $event:ident)+ $(; $($derive:ident),*)?) => {
        paste! {
            #[derive(Clone, Debug, Default $($(, $derive)*)?)] // TODO clone_from?
            pub(crate) struct [<$actor EventsFrom $src>] {
                $(pub(crate) [<$event:snake>]: <$event as Message>::Inbox),+
            }

            impl<C: ?Sized, T> Apply<[<$actor EventsFrom $src>], C> for T
            where
                T: $(
                    Apply<<$event as Message>::Inbox, C> +
                )+
            {
                fn apply(&mut self, events: &[<$actor EventsFrom $src>], context: &mut C) {
                    $(
                        self.apply(&events.[<$event:snake>], context);
                    )+
                }
            }

            $(
                impl Extend<$event> for [<$actor EventsFrom $src>] {
                    fn extend<I: IntoIterator<Item = $event>>(&mut self, i: I) {
                        self.[<$event:snake>].extend(i);
                    }
                }
            )+
        }
    }
}

// TODO is ActorState the best name? impl Actor is actually the state and this is state + inbox.
#[macro_export]
macro_rules! define_actor_state {
    ($actor:ident $(, $src:ident)* $(; $($derive:ident),*)?) => {
        paste! {
            #[derive(Debug)]
            pub struct [<$actor State>] {
                pub actor: $actor,
                pub(crate) inbox: [<$actor Inbox>],
            }

            // Can't #[derive(Default)] since actor might not be Default.
            impl<'a> Default for [<$actor State>] where TrivialBounds<'a, $actor>: Default {
                fn default() -> Self {
                    Self {
                        actor: TrivialBounds::<$actor>::default().1,
                        inbox: Default::default(),
                    }
                }
            }

            impl From<$actor> for [<$actor State>] {
                fn from(actor: $actor) -> Self {
                    Self {
                        actor,
                        inbox: Default::default(),
                    }
                }
            }

            impl<C: ?Sized, I: Message> ApplyOwned<I, C> for [<$actor State>]
            where
                $actor: Apply<I, C>, <I as Message>::Inbox: SequentialInbox,
                [<$actor EventsFromServer>]: Extend<I>,
            {
                fn apply_owned(&mut self, input: I, context: &mut C) {
                    Apply::apply(&mut self.actor, &input, context);
                    self.inbox.server.extend_one(input);
                }
            }

            #[derive(Debug, Default $($(, $derive)*)?)]
            pub struct [<$actor Inbox>] {
                $(pub(crate) [<$src:snake>]: <$src as ActorId>::Map<[<$actor EventsFrom $src>]>),*
            }

            // Optimization: #[derive(Clone)] doesn't implement clone_from.
            impl Clone for [<$actor Inbox>] {
                fn clone(&self) -> Self {
                    Self {
                        $([<$src:snake>]: self.[<$src:snake>].clone()),*
                    }
                }

                fn clone_from(&mut self, source: &Self) {
                    $(self.[<$src:snake>].clone_from(&source.[<$src:snake>]);)*
                }
            }

            impl [<$actor Inbox>] {
                pub fn filter(&self, #[allow(unused)] knowledge: &Knowledge) -> Self {
                    Self {
                        $(
                            [<$src:snake>]: Map::iter(&self.[<$src:snake>]).filter_map(|(id, events)| {
                                knowledge.is_inactive(id).then(|| {
                                    (id, events.clone())
                                })
                            }).collect(),
                        )*
                    }
                }
            }

            $(
                impl<T> Extend<($src, T)> for [<$actor Inbox>]
                    where [<$actor EventsFrom $src>]: Extend<T>
                {
                    fn extend<I: IntoIterator<Item = ($src, T)>>(&mut self, i: I) {
                        for (id, t) in i {
                            self.[<$src:snake>].or_default(id).extend_one(t);
                        }
                    }
                }
            )*
        }
    }
}

#[macro_export]
macro_rules! define_world {
    ($checksum:ty, $($actor:ident),+ $(; $($derive:ident),*)?) => {
        $crate::define_apply!();

        paste! {
            #[derive(Debug, Default)]
            pub struct World {
                $(pub [<$actor:snake>]: <<$actor as Actor>::Id as ActorId>::DenseMap<[<$actor State>]>),+
            }

            impl World {
                /// Clears all the inboxes without modifying the actual state.
                /// TODO(debug_assertions) make sure this gets called between each tick.
                pub fn post_update(&mut self) {
                    $(
                        Map::verify_ord_iter(&self.[<$actor:snake>]);
                        for actor_state in Map::values_mut(&mut self.[<$actor:snake>]) {
                            actor_state.inbox.clone_from(&Default::default());
                        }
                    )+
                }

                /// Gets an update for a client given it's knowledge.
                pub fn get_update<$([<$actor T>]: IntoIterator<Item = <$actor as Actor>::Id>), +>(
                    &self,
                    knowledge: &mut Knowledge,
                    visibility: Visibility<$(impl FnOnce(&Knowledge) -> [<$actor T>]),+>,
                ) -> ActorUpdate {
                    let mut update = ActorUpdate::default();

                    $(
                        let mut removals_len = 0;
                        Map::verify_ord_iter(&knowledge.[<$actor:snake>]);
                        for (actor_id, knowledge) in Map::iter_mut(&mut knowledge.[<$actor:snake>]) {
                            knowledge.tick(<$actor as Actor>::KEEPALIVE);
                            let remove = knowledge.is_expired() || !Map::contains(&self.[<$actor:snake>], actor_id);
                            removals_len += remove as usize;
                        }

                        let mut completes_len = 0;
                        for actor_id in (visibility.[<$actor:snake>])(&knowledge) {
                            debug_assert!(Map::contains(&self.[<$actor:snake>], actor_id), "visible actor {actor_id:?} does not exist");
                            // TODO Map::get_or_insert_with.
                            if let Some(knowledge) = Map::get_mut(&mut knowledge.[<$actor:snake>], actor_id) {
                                let before = knowledge.is_expired();
                                if knowledge.refresh(<$actor as Actor>::KEEPALIVE) {
                                    if before && !knowledge.is_expired() {
                                        removals_len -= 1;
                                    }
                                }
                            } else {
                                Map::insert(&mut knowledge.[<$actor:snake>], actor_id, Default::default());
                                completes_len += 1;
                            }
                        }

                        // Calculate exact size of Box<[T]>s to only allocate once.
                        let actor_knowledge = &mut knowledge.[<$actor:snake>];
                        let inboxes_len = Map::len(actor_knowledge) - completes_len - removals_len;
                        // println!("{:<10}: new {completes_len:>2}, alive {inboxes_len:>2}, expired {removals_len:>2}", stringify!($actor));

                        if removals_len != 0 {
                            let mut removals = Vec::with_capacity(removals_len);
                            Map::retain(actor_knowledge, |actor_id, knowledge| {
                                let remove = knowledge.is_expired() || !Map::contains(&self.[<$actor:snake>], actor_id);
                                if remove {
                                    removals.push_within_capacity(actor_id).unwrap();
                                }
                                !remove
                            });

                            debug_assert_eq!(removals.len(), removals_len);
                            update.[<$actor:snake _removals>] = removals.into_boxed_slice();
                        }

                        // Save variables for next block.
                        let [<$actor:snake _lens>] = (completes_len, inboxes_len);
                    )+

                    $(
                        // Use variables from previous block.
                        let (completes_len, inboxes_len) = [<$actor:snake _lens>];
                        let mut completes = Vec::with_capacity(completes_len);
                        let mut inboxes = Vec::with_capacity(inboxes_len);

                        for (actor_id, k) in Map::iter(&knowledge.[<$actor:snake>]) {
                            let actor_state = Map::get(&self.[<$actor:snake>], actor_id).unwrap_or_else(|| {
                                panic!("knowledge of nonexistent actor: {actor_id:?}");
                            });
                            if Checksum::is_some(&update.checksum) {
                                Accumulate::accumulate(&mut update.checksum, (actor_id, &actor_state.actor));
                            }

                            if k.is_new() {
                                completes.push_within_capacity((actor_id, actor_state.actor.clone())).unwrap();
                            } else {
                                inboxes.push_within_capacity(actor_state.inbox.filter(knowledge)).unwrap();
                            }
                        }

                        debug_assert_eq!(completes.len(), completes_len);
                        debug_assert_eq!(inboxes.len(), inboxes_len);
                        update.[<$actor:snake _completes>] = completes.into_boxed_slice();
                        update.[<$actor:snake _inboxes>] = inboxes.into_boxed_slice();
                    )+
                    update
                }

                /// Checks if `self == Self::default()` without requiring `PartialEq`.
                #[allow(unused)]
                pub fn is_default(&self) -> bool {
                    $(self.[<$actor:snake>].is_empty())&&+
                }
            }

            // Ignores messages sent to actors that aren't visible/don't exist.
            $(
                impl<T> Extend<(<$actor as Actor>::Id, T)> for World
                    where [<$actor Inbox>]: Extend<T>
                {
                    fn extend<I: IntoIterator<Item = (<$actor as Actor>::Id, T)>>(&mut self, i: I) {
                        for (dst, t) in i {
                            if let Some(actor_state) = Map::get_mut(&mut self.[<$actor:snake>], dst) {
                                actor_state.inbox.extend_one(t)
                            }
                        }
                    }
                }
            )*

            #[derive(Debug, Default $($(, $derive)*)?)]
            pub struct ActorUpdate {
                checksum: $checksum,
                $( // TODO Box may be wasteful for types like Singleton which have at most 1 entry.
                    [<$actor:snake _completes>]: Box<[(<$actor as Actor>::Id, $actor)]>,
                    [<$actor:snake _inboxes>]: Box<[[<$actor Inbox>]]>,
                    [<$actor:snake _removals>]: Box<[<$actor as Actor>::Id]>,
                )+
            }

            impl<C: ?Sized> ApplyOwned<ActorUpdate, C> for World
            where
                World: WorldTick<C>,
            {
                fn apply_owned(&mut self, update: ActorUpdate, context: &mut C) {
                    // Do removals and copy inboxes. TODO better error handling.
                    $(
                        for &removal in update.[<$actor:snake _removals>].iter() {
                            Map::remove(&mut self.[<$actor:snake>], removal).expect("removals: actor doesn't exist");
                        }

                        let actors = &mut self.[<$actor:snake>];
                        let actor_inboxes = update.[<$actor:snake _inboxes>];
                        assert_eq!(Map::len(actors), actor_inboxes.len(), "inboxes: length mismatch");

                        for (actor, inbox) in Map::values_mut(actors).zip(Vec::from(actor_inboxes)) {
                            actor.inbox = inbox;
                        }
                    )+

                    WorldTick::tick_client(self, context);

                    // Do completes.
                    $(
                        for (id, complete) in Vec::from(update.[<$actor:snake _completes>]) {
                            let previous = Map::insert(&mut self.[<$actor:snake>], id, complete.into());
                            assert!(previous.is_none(), "complete: actor already exists");
                        }
                    )+

                    let mut checksum = <$checksum>::default();
                    if Checksum::is_some(&checksum) {
                        $(
                            for (actor_id, actor_state) in Map::iter(&self.[<$actor:snake>]) {
                                Accumulate::accumulate(&mut checksum, (actor_id, &actor_state.actor));
                            }
                        )+
                    }

                    if &checksum != &update.checksum {
                        panic!("desync {}", Checksum::diff(&checksum, &update.checksum))
                    }
                }
            }

            /// What part of the world a client knows about.
            #[derive(Debug, Default)]
            pub struct Knowledge {
                $(pub [<$actor:snake>]: <<$actor as Actor>::Id as ActorId>::SparseMap<ActorKnowledge>),*
            }

            $(
                impl IsActive<<$actor as Actor>::Id> for Knowledge {
                    fn is_active(&self, id: <$actor as Actor>::Id) -> bool {
                        Map::get(&self.[<$actor:snake>], id).is_some_and(|k| k.is_active())
                    }
                }
            )+

            /// Events from [`Server`] are always sent.
            impl IsActive<Server> for Knowledge {
                fn is_active(&self, _: Server) -> bool {
                    false
                }
            }

            /// Which actors a client can see this frame. Can contain duplicates.
            pub struct Visibility<$($actor),+> {
                $(pub [<$actor:snake>]: $actor),+
            }
        }
    }
}
