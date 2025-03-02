// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

/// See https://docs.rs/actix/latest/actix/dev/trait.MessageResponse.html
#[macro_export]
macro_rules! actix_response {
    ($typ: ty) => {
        #[cfg(feature = "server")]
        impl<A, M> actix::dev::MessageResponse<A, M> for $typ
        where
            A: actix::Actor,
            M: actix::Message<Result = $typ>,
        {
            fn handle(
                self,
                _ctx: &mut A::Context,
                tx: Option<actix::dev::OneshotSender<M::Result>>,
            ) {
                if let Some(tx) = tx {
                    let _ = tx.send(self);
                }
            }
        }
    };
}
