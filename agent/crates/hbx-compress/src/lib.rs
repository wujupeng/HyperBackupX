use hbx_core::pipeline::ICompressor;

pub struct ZstdCompressor {
    level: i32,
}

impl ZstdCompressor {
    pub fn new(level: i32) -> Self {
        Self { level }
    }
}

impl Default for ZstdCompressor {
    fn default() -> Self {
        Self::new(3)
    }
}

impl ICompressor for ZstdCompressor {
    fn compress(&self, plain: &[u8]) -> Result<Vec<u8>, hbx_core::pipeline::CompressError> {
        zstd::encode_all(plain, self.level)
            .map_err(|e| hbx_core::pipeline::CompressError::Failed(e.to_string()))
    }

    fn decompress(&self, compressed: &[u8]) -> Result<Vec<u8>, hbx_core::pipeline::CompressError> {
        zstd::decode_all(compressed)
            .map_err(|e| hbx_core::pipeline::CompressError::DecompressFailed(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let compressor = ZstdCompressor::default();
        let data = b"Hello, HyperBackup X! This is a test for compression roundtrip.";
        let compressed = compressor.compress(data).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(data.as_slice(), decompressed.as_slice());
    }

    #[test]
    fn test_empty_input() {
        let compressor = ZstdCompressor::default();
        let compressed = compressor.compress(b"").unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert!(decompressed.is_empty());
    }

    #[test]
    fn test_large_input() {
        let compressor = ZstdCompressor::default();
        let data: Vec<u8> = (0..100_000).map(|i| (i % 256) as u8).collect();
        let compressed = compressor.compress(&data).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(data, decompressed);
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_compress_decompress_roundtrip(
            data in proptest::collection::vec(any::<u8>(), 0..8192)
        ) {
            let compressor = ZstdCompressor::default();
            let compressed = compressor.compress(&data).unwrap();
            let decompressed = compressor.decompress(&compressed).unwrap();
            prop_assert_eq!(decompressed, data);
        }

        #[test]
        fn prop_compress_level_roundtrip(
            data in proptest::collection::vec(any::<u8>(), 0..4096),
            level in 1i32..=22
        ) {
            let compressor = ZstdCompressor::new(level);
            let compressed = compressor.compress(&data).unwrap();
            let decompressed = compressor.decompress(&compressed).unwrap();
            prop_assert_eq!(decompressed, data);
        }
    }
}
