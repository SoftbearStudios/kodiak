use std::sync::mpsc::TryRecvError;
use std::time::{Duration, Instant};

// clear && RUST_LOG=info cargo test lockstep --features log -- --nocapture
#[test]
fn lockstep() {
    let _ = env_logger::try_init();

    use super::LockstepClient;
    use crate::bitcode::{self, *};
    use crate::{LockstepClientData, LockstepServer, LockstepWorld, PlayerId};
    use kodiak_macros::HbHash;
    extern crate self as kodiak_common;
    use rand::{thread_rng, Rng};

    #[derive(Clone, Default, Debug, Hash, Encode, Decode)]
    struct World;
    #[derive(Clone, Debug, HbHash, Encode, Decode)]
    struct Player {
        #[hb_hash]
        number: f32,
        #[hb_hash]
        velocity: f32,
    }
    #[derive(Clone, Copy, Debug, Default, HbHash, Encode, Decode)]
    struct Input {
        #[hb_hash]
        target: f32,
    }
    impl LockstepWorld for World {
        type Input = Input;
        type Player = Player;

        const BUFFERED_TICKS: usize = Self::TPS;
        const MAX_PREDICTION: usize = Self::TPS;
        const TPS: usize = 10;

        fn tick(
            &mut self,
            _tick: Self::Tick,
            context: &mut crate::lockstep::LockstepContext<Self>,
            _predicting: Option<kodiak_common::PlayerId>,
            _interpolation_prediction: bool,
            _on_info: &mut dyn FnMut(Self::Info),
        ) where
            [(); Self::LAG_COMPENSATION]:,
        {
            for (_, player) in context.players.iter_mut() {
                let velocity = ((player.input.target - player.number) * 0.4).clamp(-1.0, 1.0);
                player.velocity += (velocity - player.velocity) * 0.8;
                player.number += player.velocity * World::TICK_PERIOD_SECS;
            }
        }

        fn lerp_player(player: &Self::Player, next: &Self::Player, t: f32) -> Self::Player {
            Player {
                number: player.number + (next.number - player.number) * t,
                velocity: player.velocity + (next.velocity - player.velocity) * t,
            }
        }
    }

    let player_id = PlayerId::nth_client(0).unwrap();
    const TICKS: usize = 200;
    const TIME_SCALE: f32 = 50.0;

    let (send_to_client, client_receive) = std::sync::mpsc::channel();
    let (send_to_server, server_receive) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut rng = thread_rng();
        let mut client_data = LockstepClientData::<World>::default();
        let mut server = LockstepServer::<World>::default();
        *server.player_mut(player_id) = Some(Player {
            number: 0.0,
            velocity: 0.0,
        });
        for _ in 0..TICKS {
            if rng.gen_bool(0.8) {
                while let Ok(request) = server_receive.try_recv() {
                    server.request(player_id, request, Some(&mut client_data), false);
                }
            }
            server.update(std::iter::once((player_id, &mut client_data)));
            let client_update = server.client_update(player_id, &mut client_data);
            server.post_update();
            if send_to_client.send(client_update).is_err() {
                break;
            }
            std::thread::sleep(Duration::from_secs_f32(
                World::TICK_PERIOD_SECS * rng.gen_range(0.8..=1.2) / TIME_SCALE,
            ));
        }
    });
    let client = std::thread::spawn(move || {
        let mut rng = thread_rng();
        let mut client = LockstepClient::<World>::default();
        let mut last_time = Instant::now();
        let mut time = 0f32;
        let mut last = f32::NAN;
        const FRAMES_PER_TICK: usize = 4;
        for _ in 0..TICKS * FRAMES_PER_TICK {
            if rng.gen_bool(0.8) {
                loop {
                    match client_receive.try_recv() {
                        Ok(client_update) => {
                            let _latency = client.receive(client_update);
                        }
                        Err(TryRecvError::Disconnected) => {
                            return;
                        }
                        Err(TryRecvError::Empty) => {
                            break;
                        }
                    }
                }
            }
            let now = Instant::now();
            let elapsed_seconds = (now - last_time).as_secs_f32() * TIME_SCALE;
            last_time = now;
            let target = (time * 0.5).sin();
            let _ = client.update(
                elapsed_seconds,
                false,
                |_commit| Input { target },
                |request, _| {
                    let _ = send_to_server.send(request);
                },
            );
            if client.loaded() {
                let number = client
                    .interpolated
                    .context
                    .players
                    .get(player_id)
                    .unwrap()
                    .number;
                println!(
                    "{time:.2},{number:.3},{:.3},{target:.3},{}",
                    client
                        .predicted
                        .context
                        .players
                        .get(player_id)
                        .unwrap()
                        .input
                        .target,
                    client.interpolated.context.tick_id
                );
                if number == last {
                    panic!();
                }
                last = number;
            }

            time += elapsed_seconds;

            std::thread::sleep(Duration::from_secs_f32(
                World::TICK_PERIOD_SECS * rng.gen_range(0.9..=1.1)
                    / TIME_SCALE
                    / FRAMES_PER_TICK as f32,
            ));
        }
    });

    server.join().unwrap();
    client.join().unwrap();
}
