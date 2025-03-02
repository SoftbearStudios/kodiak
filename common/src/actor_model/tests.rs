// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

#[cfg(test)]
mod singleton_tests {
    use super::*;

    #[derive(
        Copy,
        Clone,
        Debug,
        Eq,
        PartialEq,
        Ord,
        PartialOrd,
        Hash,
        Serialize,
        Deserialize,
        Encode,
        Decode,
    )]
    pub struct SingletonId;

    impl ActorId for SingletonId {
        type Map<T> = Option<(Self, T)>;
        type SparseMap<T> = Option<(Self, T)>;
    }

    #[derive(Clone, Debug, Default, Hash, Serialize, Deserialize, Encode, Decode)]
    pub struct Singleton {
        tick: u32,
        post_tick: u32,
    }

    impl Actor for Singleton {
        type Id = SingletonId;

        const KEEPALIVE: u8 = 0;
    }

    #[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode)]
    pub enum SingletonInput {}

    impl<C> Apply<SingletonInput, C> for Singleton {
        fn apply(&mut self, _: &SingletonInput, _: &mut C) {}
    }

    impl Message for SingletonInput {}

    #[derive(
        Copy,
        Clone,
        Debug,
        Eq,
        PartialEq,
        Ord,
        PartialOrd,
        Hash,
        Serialize,
        Deserialize,
        Encode,
        Decode,
    )]
    pub struct SectorId(u32);

    impl ActorId for SectorId {
        type Map<T> = SortedVecMap<Self, T>;
        type SparseMap<T> = BTreeMap<Self, T>;
    }

    #[derive(Clone, Debug, Hash, Serialize, Deserialize, Encode, Decode)]
    pub struct Sector {
        data: Vec<u32>,
    }

    impl Actor for Sector {
        type Id = SectorId;
    }

    #[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode)]
    pub struct SectorInput {
        push: Vec<u32>,
    }

    impl<C> Apply<SectorInput, C> for Sector {
        fn apply(&mut self, input: &SectorInput, _context: &mut C) {
            self.data.extend(&input.push);
        }
    }

    impl Message for SectorInput {}

    #[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode)]
    pub struct SectorEvent {
        pop: usize,
    }

    impl<C> Apply<SectorEvent, C> for Sector {
        fn apply(&mut self, event: &SectorEvent, _context: &mut C) {
            self.data.drain(..event.pop.min(self.data.len()));
        }
    }

    impl Message for SectorEvent {}

    define_events!(Singleton, Server, SingletonInput; Serialize, Deserialize, Encode, Decode);
    define_actor_state!(Singleton, Server; Serialize, Deserialize, Encode, Decode);
    define_events!(Sector, Server, SectorInput; Serialize, Deserialize, Encode, Decode);
    define_events!(Sector, SectorId, SectorEvent; Serialize, Deserialize, Encode, Decode);
    define_actor_state!(Sector, Server, SectorId; Serialize, Deserialize, Encode, Decode);
    define_world!(u32, Singleton, Sector; Serialize, Deserialize, Encode, Decode);

    const VISIBLE_ID: SectorId = SectorId(1);
    const OTHER_ID: SectorId = SectorId(5);

    impl<C> WorldTick<C> for World {
        fn tick_before_inputs(&mut self, _: &mut C) {
            let Some(singleton) = singleton_mut!(self) else {
                return;
            };
            singleton.tick += 1;

            {
                let has_other = Map::get(&self.sector, OTHER_ID).is_some();

                if let Some(sector_state) = Map::get_mut(&mut self.sector, VISIBLE_ID) {
                    let pop = (singleton.tick / 2) as usize;

                    sector_state
                        .inbox
                        .extend_one((VISIBLE_ID, SectorEvent { pop: 1 }));

                    if has_other {
                        sector_state
                            .inbox
                            .extend_one((OTHER_ID, SectorEvent { pop }));
                    }
                }
            }

            // Don't apply events yet.
        }

        fn tick_after_inputs(&mut self, context: &mut C) {
            let Some(singleton) = singleton_mut!(self) else {
                return;
            };
            singleton.post_tick += 1;

            // Apply events.
            apply!(self, Sector, SectorId, SectorEvent, context);
        }

        fn tick_client(&mut self, context: &mut C) {
            self.tick_before_inputs(context);
            apply_inputs!(self, Singleton, SingletonInput, context);
            apply_inputs!(self, Sector, SectorInput, context);
            self.tick_after_inputs(context);
        }
    }

    #[test]
    fn test() {
        let mut world = World::default();

        Map::insert(
            &mut world.singleton,
            SingletonId,
            Singleton::default().into(),
        );
        Map::insert(
            &mut world.sector,
            VISIBLE_ID,
            Sector {
                data: vec![1, 2, 3],
            }
            .into(),
        );
        Map::insert(
            &mut world.sector,
            OTHER_ID,
            Sector { data: vec![42] }.into(),
        );

        let mut client = Knowledge::default();
        let mut client_world = World::default();

        for i in 0..10u32 {
            let tick = singleton!(world).unwrap().tick;
            println!("\nTICK {tick}");

            world.tick_before_inputs(&mut ());
            if let Some(sector_state) = Map::get_mut(&mut world.sector, VISIBLE_ID) {
                let n = tick * 3;

                sector_state.apply_owned(
                    SectorInput {
                        push: vec![n + 4, n + 5, n + 6],
                    },
                    &mut (),
                );
            }
            world.tick_after_inputs(&mut ());

            println!("server: {world:?}");

            let update = world.get_update(
                &mut client,
                Visibility {
                    singleton: |_: &_| Some(SingletonId),
                    sector: |_: &_| (tick == 0).then_some(VISIBLE_ID),
                },
            );
            world.post_update();

            let update = if i % 2 == 0 {
                bitcode::decode(&bitcode::encode(&update)).unwrap()
            } else {
                bitcode::deserialize(&bitcode::serialize(&update).unwrap()).unwrap()
            };

            println!("update: {update:?}");
            client_world.apply_owned(update, &mut ());
            println!("client: {client_world:?}");
        }
    }
}

#[cfg(test)]
mod tests2 {
    use super::*;
    use rand::prelude::IteratorRandom;
    use rand::{thread_rng, Rng};

    #[test]
    fn fuzz() {
        define_events!(Simple, Server, SimpleInput);
        define_events!(Simple, SimpleId, SimpleEvent);
        define_actor_state!(Simple, Server, SimpleId);
        define_world!(u32, Simple);

        impl WorldTick<OnInfo<'_>> for World {
            fn tick_before_inputs(&mut self, context: &mut OnInfo<'_>) {
                let mut simple_events = vec![];

                for (simple_id, actor_state) in Map::iter_mut(&mut self.simple) {
                    let actor = &mut actor_state.actor;

                    if actor.0.len() % 3 == 0 {
                        let c = 'm';
                        actor.0.push(c);
                        context(Info::CharPushed { c, new: &actor });
                    } else {
                        let c = actor.0.pop();
                        context(Info::CharPopped { c, new: &actor });
                    }

                    if simple_id.0 % 4 == 0 {
                        actor_state
                            .inbox
                            .extend_one((simple_id, SimpleEvent::Overwrite { str: "ABCDE" }));
                    }
                    if simple_id.0 % 8 == 0 {
                        actor_state
                            .inbox
                            .extend_one((simple_id, SimpleEvent::Overwrite { str: "________" }));
                    }
                    if simple_id.0 % 3 == 0 {
                        actor_state
                            .inbox
                            .extend_one((simple_id, SimpleEvent::PushChar { c: 'a' }));
                    } else if actor.0.len() % 7 == 0 {
                        actor_state
                            .inbox
                            .extend_one((simple_id, SimpleEvent::Overwrite { str: "abcd" }));
                    } else {
                        actor_state
                            .inbox
                            .extend_one((simple_id, SimpleEvent::PopChar));

                        let dst = SimpleId(simple_id.0.saturating_sub(1));
                        simple_events
                            .extend_one((dst, (simple_id, SimpleEvent::PushChar { c: 'b' })))
                    }
                }

                self.extend(simple_events);
                apply!(self, Simple, SimpleId, SimpleEvent, context);
            }

            fn tick_client(&mut self, context: &mut OnInfo<'_>) {
                self.tick_before_inputs(context);
                apply_inputs!(self, Simple, SimpleInput, context);
                self.tick_after_inputs(context);
            }
        }

        #[derive(Clone, Debug)]
        pub enum SimpleInput {
            PushChar { c: char },
        }

        impl Message for SimpleInput {}

        #[derive(Clone, Debug)]
        pub enum SimpleEvent {
            PushChar { c: char },
            PopChar,
            Overwrite { str: &'static str },
        }

        impl Message for SimpleEvent {}

        impl Apply<SimpleEvent, Dst<'_, SimpleId, OnInfo<'_>>> for Simple {
            fn apply(&mut self, event: &SimpleEvent, context: &mut Dst<'_, SimpleId, OnInfo<'_>>) {
                match event {
                    &SimpleEvent::PushChar { c, .. } => {
                        self.0.push(c);
                        context(Info::CharPushed { c, new: &self })
                    }
                    SimpleEvent::PopChar => {
                        let c = self.0.pop();
                        context(Info::CharPopped { c, new: &self });
                    }
                    SimpleEvent::Overwrite { str } => {
                        self.0.clear();
                        self.0.push_str(str);
                        context(Info::Overwritten { new: &self });
                    }
                }
            }
        }

        #[derive(Debug)]
        #[allow(unused)]
        pub enum Info<'a> {
            CharPushed { c: char, new: &'a Simple },
            CharPopped { c: Option<char>, new: &'a Simple },
            Overwritten { new: &'a Simple },
        }

        pub type OnInfo<'a> = dyn FnMut(Info<'_>) + 'a;

        #[derive(Copy, Clone, Ord, Hash, Eq, PartialEq, PartialOrd, Debug)]
        pub struct SimpleId(u8);

        impl ActorId for SimpleId {
            type Map<T> = SortedVecMap<Self, T>;
            type SparseMap<T> = BTreeMap<Self, T>;
        }

        #[derive(Clone, Hash, Debug)]
        pub struct Simple(String);

        impl Actor for Simple {
            type Id = SimpleId;
        }

        impl Apply<SimpleInput, Dst<'_, SimpleId, OnInfo<'_>>> for Simple {
            fn apply(&mut self, input: &SimpleInput, context: &mut Dst<'_, SimpleId, OnInfo<'_>>) {
                match input {
                    &SimpleInput::PushChar { c } => {
                        self.0.push(c);

                        context(Info::CharPushed { c, new: &self });
                    }
                }
            }
        }

        #[derive(Default)]
        struct Client {
            world: World,
            data: Knowledge,
        }

        fn update_clients(server: &World, clients: &mut [Client], context: &mut OnInfo<'_>) {
            let n_clients = clients.len();
            for (i, client) in clients.iter_mut().enumerate() {
                let update = server.get_update(
                    &mut client.data,
                    Visibility {
                        simple: |_: &_| {
                            Map::iter(&server.simple).map(|(k, _)| k).filter(move |&n| {
                                thread_rng().gen_bool(if n.0 as usize % n_clients == i {
                                    0.9
                                } else {
                                    0.1
                                })
                            })
                        },
                    },
                );
                client.world.apply_owned(update, context);
            }
        }

        let mut rng = thread_rng();
        let isolate = false;

        #[derive(Default)]
        struct Context;

        const DEBUG: bool = false;
        let context: &mut OnInfo<'_> = &mut |i: Info| {
            if DEBUG {
                println!("Info: {i:?}");
            }
        };

        for i in 0..512 {
            if DEBUG {
                println!("@@@@@@@@@@@@@@@@@@@@@@@@ FUZZ #{i}");
            }

            let mut server = World::default();
            let mut clients = std::iter::repeat_with(Client::default)
                .take(if isolate { 1 } else { rng.gen_range(0..=32) })
                .collect::<Vec<_>>();

            let mut possible_ids = if isolate {
                vec![22, 23]
            } else {
                (0..32).collect::<Vec<_>>()
            };

            for j in 0..rng.gen_range(1..=16) {
                if DEBUG {
                    println!("@@@@@@@@@@@@@@@ ITERATION #{j}");
                    println!("@@@@@@@ DISPATCH");
                }

                for _ in 0..rng.gen_range(0..=4) {
                    if possible_ids.is_empty() {
                        break;
                    }

                    let i = rng.gen_range(0..possible_ids.len());
                    let id = possible_ids.swap_remove(i);
                    server
                        .simple
                        .insert(SimpleId(id), Simple(i.to_string()).into());
                }

                if DEBUG {
                    println!("@@@@@@@ DISPATCH 2");
                }

                if !Map::is_empty(&server.simple) {
                    for _ in 0..rng.gen_range(0..=if isolate { 3 } else { 25 }) {
                        let (id, v) = Map::iter_mut(&mut server.simple).choose(&mut rng).unwrap();

                        let mut context = Dst::new(context, id);
                        v.apply_owned(
                            SimpleInput::PushChar {
                                c: rng.gen_range('0'..='9'),
                            },
                            &mut context,
                        );
                    }
                }
                server.tick_after_inputs(context);

                if DEBUG {
                    println!("@@@@@@@ UPDATE CLIENTS");
                }

                update_clients(&mut server, &mut clients, context);
                server.post_update();

                if DEBUG {
                    println!("@@@@@@@ TICK: {server:?}");
                }

                server.tick_before_inputs(context);
            }
        }
    }
}
