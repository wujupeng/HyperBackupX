use futures::stream::Stream;
use hbx_core::pipeline::{ChunkError, ChunkStrategy, IChunker, RawChunkData};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

pub struct FixedChunker;

impl FixedChunker {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FixedChunker {
    fn default() -> Self {
        Self::new()
    }
}

impl IChunker for FixedChunker {
    fn chunk(
        &self,
        reader: Box<dyn AsyncRead + Send + Unpin>,
        strategy: ChunkStrategy,
    ) -> Result<Box<dyn Stream<Item = RawChunkData> + Send + Unpin>, ChunkError> {
        let chunk_size = match strategy {
            ChunkStrategy::Fixed { chunk_size } => chunk_size,
            ChunkStrategy::Cdc { avg_size, .. } => avg_size,
        };

        if chunk_size == 0 {
            return Err(ChunkError::InvalidSize(chunk_size));
        }

        let (tx, rx) = mpsc::channel::<RawChunkData>(4);
        let chunk_size_usize = chunk_size as usize;

        tokio::spawn(async move {
            let mut reader = reader;
            let mut offset: u64 = 0;
            let mut buf = vec![0u8; chunk_size_usize];

            loop {
                match reader.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        let chunk = RawChunkData {
                            offset,
                            data: buf[..n].to_vec(),
                        };
                        offset += n as u64;
                        if tx.send(chunk).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::error!("chunk read error: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(Box::new(ReceiverStream::new(rx)))
    }
}

pub struct FastCdcChunker {
    min_size: usize,
    avg_size: usize,
    max_size: usize,
}

impl FastCdcChunker {
    pub fn new(min_size: usize, avg_size: usize, max_size: usize) -> Self {
        Self {
            min_size,
            avg_size,
            max_size,
        }
    }

    pub fn default_for_modern() -> Self {
        Self::new(4 * 1024 * 1024, 8 * 1024 * 1024, 64 * 1024 * 1024)
    }
}

impl IChunker for FastCdcChunker {
    fn chunk(
        &self,
        reader: Box<dyn AsyncRead + Send + Unpin>,
        _strategy: ChunkStrategy,
    ) -> Result<Box<dyn Stream<Item = RawChunkData> + Send + Unpin>, ChunkError> {
        let min_size = self.min_size;
        let avg_size = self.avg_size;
        let max_size = self.max_size;

        let (tx, rx) = mpsc::channel::<RawChunkData>(4);

        tokio::spawn(async move {
            let mut reader = reader;
            let mut offset: u64 = 0;
            let mut buf = Vec::with_capacity(max_size);
            let mut read_buf = vec![0u8; avg_size];

            loop {
                match reader.read(&mut read_buf).await {
                    Ok(0) => {
                        if !buf.is_empty() {
                            let chunk = RawChunkData {
                                offset,
                                data: std::mem::take(&mut buf),
                            };
                            let _ = tx.send(chunk).await;
                        }
                        break;
                    }
                    Ok(n) => {
                        buf.extend_from_slice(&read_buf[..n]);
                        while buf.len() >= min_size {
                            let boundary = find_cdc_boundary(&buf, min_size, avg_size, max_size);
                            if boundary < buf.len() {
                                let chunk_data = buf[..boundary].to_vec();
                                buf.drain(..boundary);
                                let chunk = RawChunkData {
                                    offset,
                                    data: chunk_data,
                                };
                                offset += boundary as u64;
                                if tx.send(chunk).await.is_err() {
                                    return;
                                }
                            } else {
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("cdc chunk read error: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(Box::new(ReceiverStream::new(rx)))
    }
}

fn find_cdc_boundary(data: &[u8], min_size: usize, avg_size: usize, max_size: usize) -> usize {
    if data.len() <= min_size {
        return data.len();
    }

    let mask = compute_mask(avg_size);
    let start = min_size;
    let end = data.len().min(max_size);

    let mut hash: u64 = 0;
    for (i, &byte) in data.iter().enumerate().take(end).skip(start) {
        hash = hash.wrapping_shl(1).wrapping_add(byte as u64);
        if (hash & mask) == 0 {
            return i + 1;
        }
    }

    end
}

fn compute_mask(avg_size: usize) -> u64 {
    let bits = (avg_size as f64).log2().round() as u32;
    if bits >= 64 {
        0
    } else {
        (!0u64) >> bits
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use std::io::Cursor;
    use tokio::io::BufReader;

    fn make_reader(data: Vec<u8>) -> Box<dyn AsyncRead + Send + Unpin> {
        Box::new(BufReader::new(Cursor::new(data)))
    }

    #[tokio::test]
    async fn test_fixed_chunker_roundtrip() {
        let data = vec![0xAB; 10 * 1024 * 1024];
        let chunker = FixedChunker::new();
        let strategy = ChunkStrategy::Fixed {
            chunk_size: 1024 * 1024,
        };

        let mut stream = chunker.chunk(make_reader(data.clone()), strategy).unwrap();
        let mut reconstructed = Vec::new();
        let mut chunk_count = 0;

        while let Some(chunk) = stream.next().await {
            reconstructed.extend_from_slice(&chunk.data);
            chunk_count += 1;
        }

        assert_eq!(reconstructed, data);
        assert_eq!(chunk_count, 10);
    }

    #[tokio::test]
    async fn test_fixed_chunker_partial_last() {
        let data = vec![0xCD; 3 * 1024 * 1024 + 500];
        let chunker = FixedChunker::new();
        let strategy = ChunkStrategy::Fixed {
            chunk_size: 1024 * 1024,
        };

        let mut stream = chunker.chunk(make_reader(data.clone()), strategy).unwrap();
        let mut reconstructed = Vec::new();
        let mut chunk_count = 0;

        while let Some(chunk) = stream.next().await {
            reconstructed.extend_from_slice(&chunk.data);
            chunk_count += 1;
        }

        assert_eq!(reconstructed, data);
        assert_eq!(chunk_count, 4);
    }

    #[tokio::test]
    async fn test_fixed_chunker_empty() {
        let data = vec![];
        let chunker = FixedChunker::new();
        let strategy = ChunkStrategy::Fixed {
            chunk_size: 1024 * 1024,
        };

        let stream = chunker.chunk(make_reader(data), strategy).unwrap();
        let count: usize = stream.count().await;
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_fixed_chunker_offsets() {
        let data = vec![0xEF; 5 * 1024 * 1024];
        let chunker = FixedChunker::new();
        let strategy = ChunkStrategy::Fixed {
            chunk_size: 1024 * 1024,
        };

        let mut stream = chunker.chunk(make_reader(data), strategy).unwrap();
        let mut expected_offset = 0u64;

        while let Some(chunk) = stream.next().await {
            assert_eq!(chunk.offset, expected_offset);
            expected_offset += chunk.data.len() as u64;
        }
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use futures::StreamExt;
    use proptest::prelude::*;
    use std::io::Cursor;
    use tokio::io::BufReader;

    fn make_reader(data: Vec<u8>) -> Box<dyn AsyncRead + Send + Unpin> {
        Box::new(BufReader::new(Cursor::new(data)))
    }

    proptest! {
        #[test]
        fn prop_chunk_concat_equals_original(
            data in proptest::collection::vec(any::<u8>(), 0..16384),
            chunk_size in 1u64..=8192
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let reconstructed = rt.block_on(async {
                let chunker = FixedChunker::new();
                let strategy = ChunkStrategy::Fixed { chunk_size };

                let mut stream = chunker.chunk(make_reader(data.clone()), strategy).unwrap();
                let mut reconstructed = Vec::new();

                while let Some(chunk) = stream.next().await {
                    reconstructed.extend_from_slice(&chunk.data);
                }
                reconstructed
            });

            prop_assert_eq!(reconstructed, data);
        }
    }
}
