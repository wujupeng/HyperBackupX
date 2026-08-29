use serde::{Deserialize, Serialize};

use crate::pipeline::ChunkStrategy;

const KB: u64 = 1024;
const MB: u64 = 1024 * 1024;
const GB: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChunkingProfile {
    Small,
    Standard,
    Large,
    Adaptive,
}

impl Default for ChunkingProfile {
    fn default() -> Self {
        ChunkingProfile::Standard
    }
}

impl std::fmt::Display for ChunkingProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChunkingProfile::Small => write!(f, "Small"),
            ChunkingProfile::Standard => write!(f, "Standard"),
            ChunkingProfile::Large => write!(f, "Large"),
            ChunkingProfile::Adaptive => write!(f, "Adaptive"),
        }
    }
}

impl std::str::FromStr for ChunkingProfile {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Small" | "small" => Ok(ChunkingProfile::Small),
            "Standard" | "standard" => Ok(ChunkingProfile::Standard),
            "Large" | "large" => Ok(ChunkingProfile::Large),
            "Adaptive" | "adaptive" => Ok(ChunkingProfile::Adaptive),
            _ => Err(format!("invalid chunking profile: {}", s)),
        }
    }
}

impl ChunkingProfile {
    pub fn to_chunk_strategy(&self, file_size: u64) -> ChunkStrategy {
        match self {
            ChunkingProfile::Small => ChunkStrategy::Fixed { chunk_size: 256 * KB },
            ChunkingProfile::Standard => ChunkStrategy::Fixed { chunk_size: 512 * KB },
            ChunkingProfile::Large => ChunkStrategy::Fixed { chunk_size: 1 * MB },
            ChunkingProfile::Adaptive => {
                if file_size < 16 * MB {
                    ChunkStrategy::Fixed { chunk_size: 256 * KB }
                } else if file_size < 256 * MB {
                    ChunkStrategy::Fixed { chunk_size: 512 * KB }
                } else if file_size < 4 * GB {
                    ChunkStrategy::Fixed { chunk_size: 1 * MB }
                } else {
                    ChunkStrategy::Cdc {
                        min_size: 4 * MB,
                        avg_size: 8 * MB,
                        max_size: 64 * MB,
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_small_profile() {
        let profile = ChunkingProfile::Small;
        let strategy = profile.to_chunk_strategy(0);
        assert_eq!(strategy, ChunkStrategy::Fixed { chunk_size: 256 * KB });
    }

    #[test]
    fn test_standard_profile() {
        let profile = ChunkingProfile::Standard;
        let strategy = profile.to_chunk_strategy(0);
        assert_eq!(strategy, ChunkStrategy::Fixed { chunk_size: 512 * KB });
    }

    #[test]
    fn test_large_profile() {
        let profile = ChunkingProfile::Large;
        let strategy = profile.to_chunk_strategy(0);
        assert_eq!(strategy, ChunkStrategy::Fixed { chunk_size: 1 * MB });
    }

    #[test]
    fn test_adaptive_small_file() {
        let profile = ChunkingProfile::Adaptive;
        let strategy = profile.to_chunk_strategy(8 * MB);
        assert_eq!(strategy, ChunkStrategy::Fixed { chunk_size: 256 * KB });
    }

    #[test]
    fn test_adaptive_medium_file() {
        let profile = ChunkingProfile::Adaptive;
        let strategy = profile.to_chunk_strategy(128 * MB);
        assert_eq!(strategy, ChunkStrategy::Fixed { chunk_size: 512 * KB });
    }

    #[test]
    fn test_adaptive_large_file() {
        let profile = ChunkingProfile::Adaptive;
        let strategy = profile.to_chunk_strategy(1 * GB);
        assert_eq!(strategy, ChunkStrategy::Fixed { chunk_size: 1 * MB });
    }

    #[test]
    fn test_adaptive_huge_file() {
        let profile = ChunkingProfile::Adaptive;
        let strategy = profile.to_chunk_strategy(8 * GB);
        assert_eq!(
            strategy,
            ChunkStrategy::Cdc {
                min_size: 4 * MB,
                avg_size: 8 * MB,
                max_size: 64 * MB,
            }
        );
    }

    #[test]
    fn test_default_is_standard() {
        assert_eq!(ChunkingProfile::default(), ChunkingProfile::Standard);
    }

    #[test]
    fn test_from_str() {
        assert_eq!("Small".parse::<ChunkingProfile>().unwrap(), ChunkingProfile::Small);
        assert_eq!("standard".parse::<ChunkingProfile>().unwrap(), ChunkingProfile::Standard);
        assert_eq!("Large".parse::<ChunkingProfile>().unwrap(), ChunkingProfile::Large);
        assert_eq!("adaptive".parse::<ChunkingProfile>().unwrap(), ChunkingProfile::Adaptive);
        assert!("invalid".parse::<ChunkingProfile>().is_err());
    }

    #[test]
    fn test_display() {
        assert_eq!(ChunkingProfile::Small.to_string(), "Small");
        assert_eq!(ChunkingProfile::Standard.to_string(), "Standard");
        assert_eq!(ChunkingProfile::Large.to_string(), "Large");
        assert_eq!(ChunkingProfile::Adaptive.to_string(), "Adaptive");
    }
}