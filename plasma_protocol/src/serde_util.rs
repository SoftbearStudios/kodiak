// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use serde::Deserializer;

pub fn box_slice_skip_invalid<'de, T: serde::de::DeserializeOwned, D>(
    deserializer: D,
) -> Result<Box<[T]>, D::Error>
where
    D: Deserializer<'de>,
{
    struct SeqVisitor<T>(std::marker::PhantomData<T>);

    impl<'de, T: serde::de::DeserializeOwned> serde::de::Visitor<'de> for SeqVisitor<T> {
        type Value = Box<[T]>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a sequence")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            let mut values = Vec::with_capacity(seq.size_hint().unwrap_or_default().min(32));
            // Cannot used untagged enum due to https://github.com/serde-rs/serde/issues/2672
            while let Some(value) = seq.next_element::<&serde_json::value::RawValue>()? {
                if let Ok(value) = serde_json::from_str(value.get()) {
                    values.push(value);
                } else {
                    #[cfg(feature = "server")]
                    log::warn!("failed to deserialize {value}");
                }
            }
            Ok(values.into_boxed_slice())
        }
    }

    let visitor = SeqVisitor::<T>(std::marker::PhantomData);
    deserializer.deserialize_seq(visitor)
}
