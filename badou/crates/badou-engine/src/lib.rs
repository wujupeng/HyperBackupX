//! 八斗核心引擎：七种对象编排。

pub mod domain;
pub mod format;

#[cfg(test)]
mod tests {
    use hbx_core::domain::chunk::ChunkHash;

    #[test]
    fn shared_kernel_chunk_hash_accessible() {
        let hash = ChunkHash([0u8; 32]);
        assert_eq!(hash.0, [0u8; 32]);
    }
}
