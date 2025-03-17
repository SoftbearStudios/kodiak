// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

macro_rules! impl_deref {
    ($t:ident) => {
        impl<T, C: ?Sized> std::ops::Deref for $t<'_, T, C> {
            type Target = C;

            fn deref(&self) -> &Self::Target {
                &*self.1
            }
        }

        impl<T, C: ?Sized> std::ops::DerefMut for $t<'_, T, C> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut *self.1
            }
        }
    };
}

pub struct Src<'a, ID, C: ?Sized>(ID, &'a mut C);
impl_deref!(Src);

impl<'a, ID: Copy, C: ?Sized> Src<'a, ID, C> {
    pub fn new(context: &'a mut C, src: ID) -> Self {
        Self(src, context)
    }

    pub fn src(&self) -> ID {
        self.0
    }
}

pub struct Dst<'a, ID, C: ?Sized>(ID, &'a mut C);
impl_deref!(Dst);

impl<'a, ID: Copy, C: ?Sized> Dst<'a, ID, C> {
    pub fn new(context: &'a mut C, dst: ID) -> Self {
        Self(dst, context)
    }

    pub fn dst(&self) -> ID {
        self.0
    }
}

#[macro_export]
macro_rules! define_on {
    ($src:ident, $dst:ident, $event:ident) => {
        $crate::actor_model::paste! {
            pub struct [<On $event>]<'a, C: ?Sized>(Vec<($dst, ($src, $event))>, &'a mut C);

            impl<C: ?Sized> std::ops::Deref for [<On $event>]<'_, C> {
                type Target = C;

                fn deref(&self) -> &Self::Target {
                    &self.1
                }
            }

            impl<C: ?Sized> std::ops::DerefMut for [<On $event>]<'_, C> {
                fn deref_mut(&mut self) -> &mut Self::Target {
                    &mut self.1
                }
            }

            impl<'a, C: ?Sized> [<On $event>]<'a, C> {
                pub fn new(context: &'a mut C) -> Self {
                    Self(vec![], context)
                }

                pub fn [<on_ $event:snake>](&mut self, src: $src, dst: $dst, event: $event) {
                    self.0.push((dst, (src, event)));
                }

                pub fn into_events(self) -> Vec<($dst, ($src, $event))> {
                    self.0
                }
            }
        }
    };
}
