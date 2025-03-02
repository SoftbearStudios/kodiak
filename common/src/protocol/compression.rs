// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

#[allow(unused)]
use std::io::{Read, Write};

#[cfg(not(any(feature = "lz4", feature = "zstd")))]
pub type CompressionImpl = Uncompressed;
#[cfg(feature = "lz4")]
pub type CompressionImpl = Lz4;
#[cfg(feature = "zstd")]
pub type CompressionImpl = Zstd<16>;

pub trait Compression {
    type Compressor: Compressor;
    type Decompressor: Decompressor;

    fn compress(uncompressed: &[u8]) -> Vec<u8>;
    fn decompress(compressed: &[u8]) -> Result<Vec<u8>, ()>;
}

pub trait Compressor: Default {
    fn compress(&mut self, uncompressed: &[u8]) -> Vec<u8>;
}

pub trait Decompressor: Default {
    fn decompress(&mut self, compressed: &[u8]) -> Result<Vec<u8>, ()>;
}

#[derive(Default)]
pub struct Uncompressed;

impl Compression for Uncompressed {
    type Compressor = Self;
    type Decompressor = Self;

    fn compress(uncompressed: &[u8]) -> Vec<u8> {
        uncompressed.to_owned()
    }

    fn decompress(compressed: &[u8]) -> Result<Vec<u8>, ()> {
        Ok(compressed.to_owned())
    }
}

impl Compressor for Uncompressed {
    fn compress(&mut self, uncompressed: &[u8]) -> Vec<u8> {
        uncompressed.to_owned()
    }
}

impl Decompressor for Uncompressed {
    fn decompress(&mut self, compressed: &[u8]) -> Result<Vec<u8>, ()> {
        Ok(compressed.to_owned())
    }
}

#[cfg(any(test, feature = "lz4"))]
pub use _lz4_mod::*;
#[cfg(any(test, feature = "lz4"))]
mod _lz4_mod {
    use super::*;
    pub struct Lz4;

    impl Compression for Lz4 {
        type Compressor = Lz4Compressor;
        type Decompressor = Lz4Decompressor;

        fn compress(uncompressed: &[u8]) -> Vec<u8> {
            lz4_flex::compress_prepend_size(uncompressed)
        }

        fn decompress(compressed: &[u8]) -> Result<Vec<u8>, ()> {
            lz4_flex::decompress_size_prepended(compressed).map_err(|_| ())
        }
    }

    pub struct Lz4Compressor {
        inner: lz4_flex::frame::FrameEncoder<Vec<u8>>,
    }

    pub struct Lz4Decompressor {
        inner: lz4_flex::frame::FrameDecoder<std::io::Cursor<Vec<u8>>>,
    }

    impl Default for Lz4Compressor {
        fn default() -> Self {
            Self {
                inner: lz4_flex::frame::FrameEncoder::with_frame_info(
                    lz4_flex::frame::FrameInfo::new()
                        .block_mode(lz4_flex::frame::BlockMode::Linked)
                        .block_size(lz4_flex::frame::BlockSize::Max64KB),
                    Default::default(),
                ),
            }
        }
    }

    impl Compressor for Lz4Compressor {
        fn compress(&mut self, uncompressed: &[u8]) -> Vec<u8> {
            self.inner.write_all(uncompressed).unwrap();
            self.inner.flush().unwrap();
            std::mem::take(&mut self.inner.get_mut())
        }
    }

    impl Default for Lz4Decompressor {
        fn default() -> Self {
            Self {
                inner: lz4_flex::frame::FrameDecoder::new(Default::default()),
            }
        }
    }

    impl Decompressor for Lz4Decompressor {
        fn decompress(&mut self, compressed: &[u8]) -> Result<Vec<u8>, ()> {
            *self.inner.get_mut() = std::io::Cursor::new(compressed.to_owned());
            let mut ret = Vec::new();
            self.inner.read_to_end(&mut ret).map_err(|_| ())?;
            Ok(ret)
        }
    }
}

#[cfg(any(feature = "zstd"))]
pub use _zstd_mod::*;
#[cfg(any(feature = "zstd"))]
mod _zstd_mod {
    use super::*;
    pub struct Zstd<const W: u32>;

    impl<const W: u32> Compression for Zstd<W> {
        type Compressor = ZstdCompressor<W>;
        type Decompressor = ZstdDecompressor<W>;

        // TODO: compress, decompress
    }

    pub struct ZstdCompressor<const W: u32> {
        inner: zstd::stream::Encoder<'static, Vec<u8>>,
    }

    pub struct ZstdDecompressor<const W: u32> {
        inner: zstd::stream::write::Decoder<'static, Vec<u8>>,
    }

    impl<const W: u32> Default for ZstdCompressor<W> {
        fn default() -> Self {
            let mut inner = zstd::Encoder::<Vec<u8>>::new(Default::default(), 0).unwrap();
            inner.include_magicbytes(false).unwrap();
            if W <= 30 {
                inner.window_log(W).unwrap();
            }
            Self { inner }
        }
    }

    impl<const W: u32> Compressor for ZstdCompressor<W> {
        fn compress(&mut self, uncompressed: &[u8]) -> Vec<u8> {
            self.inner.write_all(&uncompressed).unwrap();
            self.inner.flush().unwrap();
            std::mem::take(self.inner.get_mut())
        }
    }

    impl<const W: u32> Drop for ZstdCompressor<W> {
        fn drop(&mut self) {
            // docs say to do this, although it probably doesn't do anything.
            let _ = self.inner.do_finish();
        }
    }

    impl<const W: u32> Default for ZstdDecompressor<W> {
        fn default() -> Self {
            let mut inner = zstd::stream::write::Decoder::new(Default::default()).unwrap();
            inner.include_magicbytes(false).unwrap();
            if W <= 30 {
                inner.window_log_max(W).unwrap();
            }
            Self { inner }
        }
    }

    impl<const W: u32> Decompressor for ZstdDecompressor<W> {
        fn decompress(&mut self, compressed: &[u8]) -> Result<Vec<u8>, ()> {
            self.inner.write_all(&compressed).map_err(|_| ())?;
            self.inner.flush().map_err(|_| ())?;
            Ok(std::mem::take(self.inner.get_mut()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Compression, Compressor, Decompressor, Lz4, Uncompressed};
    use rand::prelude::*;

    #[test]
    fn uncompressed() {
        test_compression::<Uncompressed>();
    }

    #[test]
    fn lz4() {
        test_compression::<Lz4>();
    }

    /*
    #[test]
    fn zstd() {
        // get some idea about the default window log.
        let mut default_c = <Zstd<21> as Compression>::Compressor::default();
        let mut default_c2 = <Zstd<40> as Compression>::Compressor::default();
        let mut rng = thread_rng();
        let big_data = std::iter::repeat_with(|| rng.gen_range(0u8..=2))
            .take(1000000)
            .collect::<Vec<_>>();
        assert_eq!(
            default_c.compress(&big_data),
            default_c2.compress(&big_data)
        );

        test_compression::<Zstd<10>>();
        test_compression::<Zstd<20>>();
        test_compression::<Zstd<30>>();
        test_compression::<Zstd<31>>();
    }
    */

    fn test_compression<C: Compression>() {
        let mut c = C::Compressor::default();
        let mut d = C::Decompressor::default();
        let mut rng = thread_rng();
        for _ in 0..10 {
            let len = rng.gen_range(200..=600);
            let input = std::iter::repeat_with(|| rng.gen_range(0u8..=2))
                .take(len)
                .collect::<Vec<_>>();
            let compressed = c.compress(&input);
            let decompressed = d.decompress(&compressed).unwrap();
            assert_eq!(input, decompressed);
        }
    }
}
