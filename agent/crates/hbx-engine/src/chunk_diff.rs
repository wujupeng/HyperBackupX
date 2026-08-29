use futures::stream::StreamExt;
use hbx_core::domain::chunk::ChunkHash;
use hbx_core::pipeline::{ChunkError, ChunkStrategy, IChunker, RawChunkData};

#[derive(Debug, Clone)]
pub struct ChunkDiff {
    pub new_chunks: Vec<RawChunkData>,
    pub reused_chunks: Vec<RawChunkData>,
    pub reused_chunk_refs: Vec<ChunkHash>,
}

impl ChunkDiff {
    pub async fn compute(
        chunker: &dyn IChunker,
        reader: Box<dyn tokio::io::AsyncRead + Send + Unpin>,
        baseline_chunks: &[ChunkHash],
        strategy: ChunkStrategy,
    ) -> Result<ChunkDiff, ChunkError> {
        let chunk_stream = chunker.chunk(reader, strategy)?;

        let mut new_chunks = Vec::new();
        let mut reused_chunks = Vec::new();
        let mut reused_chunk_refs = Vec::new();
        let mut alignment_broken = false;

        tokio::pin!(chunk_stream);
        let mut chunk_index = 0;

        while let Some(chunk) = chunk_stream.next().await {
            let hash = compute_chunk_hash(&chunk.data);

            if !alignment_broken && chunk_index < baseline_chunks.len() {
                if hash == baseline_chunks[chunk_index] {
                    reused_chunks.push(chunk.clone());
                    reused_chunk_refs.push(hash);
                } else {
                    alignment_broken = true;
                    new_chunks.push(chunk);
                }
            } else {
                new_chunks.push(chunk);
            }

            chunk_index += 1;
        }

        Ok(ChunkDiff {
            new_chunks,
            reused_chunks,
            reused_chunk_refs,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.new_chunks.is_empty() && self.reused_chunk_refs.is_empty()
    }

    pub fn all_chunks(&self) -> Vec<&RawChunkData> {
        let mut result = Vec::with_capacity(self.new_chunks.len() + self.reused_chunks.len());
        result.extend(self.new_chunks.iter());
        result.extend(self.reused_chunks.iter());
        result
    }
}

fn compute_chunk_hash(data: &[u8]) -> ChunkHash {
    let hash = blake3::hash(data);
    ChunkHash(*hash.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbx_chunker::FixedChunker;
    use hbx_core::pipeline::ChunkStrategy;
    use std::io::Cursor;

    fn make_chunk_hash(data: &[u8]) -> ChunkHash {
        let hash = blake3::hash(data);
        ChunkHash(*hash.as_bytes())
    }

    #[tokio::test]
    async fn test_all_chunks_reused() {
        let chunker = FixedChunker::new();
        let data = vec![0xabu8; 4096];
        let reader = Box::new(Cursor::new(data.clone()));
        let strategy = ChunkStrategy::Fixed { chunk_size: 1024 };

        let baseline_chunks: Vec<ChunkHash> = vec![
            make_chunk_hash(&[0xab; 1024]),
            make_chunk_hash(&[0xab; 1024]),
            make_chunk_hash(&[0xab; 1024]),
            make_chunk_hash(&[0xab; 1024]),
        ];

        let diff = ChunkDiff::compute(&chunker, reader, &baseline_chunks, strategy)
            .await
            .unwrap();

        assert_eq!(diff.new_chunks.len(), 0);
        assert_eq!(diff.reused_chunk_refs.len(), 4);
        assert_eq!(diff.reused_chunks.len(), 4);
    }

    #[tokio::test]
    async fn test_all_chunks_new() {
        let chunker = FixedChunker::new();
        let data = vec![0xcdu8; 4096];
        let reader = Box::new(Cursor::new(data.clone()));
        let strategy = ChunkStrategy::Fixed { chunk_size: 1024 };

        let baseline_chunks: Vec<ChunkHash> = vec![
            make_chunk_hash(&[0xab; 1024]),
            make_chunk_hash(&[0xab; 1024]),
            make_chunk_hash(&[0xab; 1024]),
            make_chunk_hash(&[0xab; 1024]),
        ];

        let diff = ChunkDiff::compute(&chunker, reader, &baseline_chunks, strategy)
            .await
            .unwrap();

        assert_eq!(diff.new_chunks.len(), 4);
        assert_eq!(diff.reused_chunk_refs.len(), 0);
    }

    #[tokio::test]
    async fn test_partial_chunk_reuse() {
        let chunker = FixedChunker::new();
        let mut data = vec![0xabu8; 4096];
        data[2048..3072].fill(0xcd);
        let reader = Box::new(Cursor::new(data.clone()));
        let strategy = ChunkStrategy::Fixed { chunk_size: 1024 };

        let baseline_chunks: Vec<ChunkHash> = vec![
            make_chunk_hash(&[0xab; 1024]),
            make_chunk_hash(&[0xab; 1024]),
            make_chunk_hash(&[0xab; 1024]),
            make_chunk_hash(&[0xab; 1024]),
        ];

        let diff = ChunkDiff::compute(&chunker, reader, &baseline_chunks, strategy)
            .await
            .unwrap();

        assert_eq!(diff.reused_chunk_refs.len(), 2);
        assert_eq!(diff.new_chunks.len(), 2);
    }

    #[tokio::test]
    async fn test_alignment_shift_all_new() {
        let chunker = FixedChunker::new();
        let mut data = vec![0xabu8; 4096];
        data[500..600].fill(0xcd);
        let reader = Box::new(Cursor::new(data.clone()));
        let strategy = ChunkStrategy::Fixed { chunk_size: 1024 };

        let baseline_chunks: Vec<ChunkHash> = vec![
            make_chunk_hash(&[0xab; 1024]),
            make_chunk_hash(&[0xab; 1024]),
            make_chunk_hash(&[0xab; 1024]),
            make_chunk_hash(&[0xab; 1024]),
        ];

        let diff = ChunkDiff::compute(&chunker, reader, &baseline_chunks, strategy)
            .await
            .unwrap();

        assert_eq!(diff.reused_chunk_refs.len(), 0);
        assert_eq!(diff.new_chunks.len(), 4);
    }

    #[tokio::test]
    async fn test_empty_baseline_all_new() {
        let chunker = FixedChunker::new();
        let data = vec![0xabu8; 2048];
        let reader = Box::new(Cursor::new(data.clone()));
        let strategy = ChunkStrategy::Fixed { chunk_size: 1024 };

        let diff = ChunkDiff::compute(&chunker, reader, &[], strategy)
            .await
            .unwrap();

        assert_eq!(diff.new_chunks.len(), 2);
        assert_eq!(diff.reused_chunk_refs.len(), 0);
    }

    #[tokio::test]
    async fn test_more_chunks_than_baseline() {
        let chunker = FixedChunker::new();
        let data = vec![0xabu8; 4096];
        let reader = Box::new(Cursor::new(data.clone()));
        let strategy = ChunkStrategy::Fixed { chunk_size: 1024 };

        let baseline_chunks: Vec<ChunkHash> = vec![
            make_chunk_hash(&[0xab; 1024]),
            make_chunk_hash(&[0xab; 1024]),
        ];

        let diff = ChunkDiff::compute(&chunker, reader, &baseline_chunks, strategy)
            .await
            .unwrap();

        assert_eq!(diff.reused_chunk_refs.len(), 2);
        assert_eq!(diff.new_chunks.len(), 2);
    }
}