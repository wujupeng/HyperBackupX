use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn main() {
    let args: Vec<String> = env::args().collect();
    let phase = args.get(1).map(|s| s.as_str()).unwrap_or("idle");
    let duration_sec: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(30);
    let server = args.get(3).map(|s| s.as_str()).unwrap_or("192.168.2.3:9090");

    eprintln!("[agent-sim] phase={} duration={}s server={}", phase, duration_sec, server);

    let tmp_dir = PathBuf::from(env::var("TEMP").unwrap_or("C:\\temp".to_string()));
    let work_dir = tmp_dir.join("badou-agent-sim");
    fs::create_dir_all(&work_dir).unwrap();

    let start = Instant::now();
    let mut chunk_count = 0u64;
    let mut total_bytes = 0u64;

    while start.elapsed() < Duration::from_secs(duration_sec) {
        match phase {
            "idle" => {
                std::thread::sleep(Duration::from_millis(100));
            }
            "backup" => {
                let data = vec![0xABu8; 65536];
                let hash = blake3::hash(&data);
                let hash_hex = hex::encode(hash.as_bytes());
                let chunk_file = work_dir.join(format!("chunk_{}.bin", chunk_count));
                let mut f = fs::File::create(&chunk_file).unwrap();
                f.write_all(&data).unwrap();
                total_bytes += data.len() as u64;
                chunk_count += 1;
                eprintln!("[agent-sim] backup chunk {} hash={}.. size={}", chunk_count, &hash_hex[..16], data.len());
                std::thread::sleep(Duration::from_millis(50));
            }
            "incremental" => {
                let data = vec![0xCDu8; 4096];
                let hash = blake3::hash(&data);
                let hash_hex = hex::encode(hash.as_bytes());
                let chunk_file = work_dir.join(format!("incr_{}.bin", chunk_count));
                let mut f = fs::File::create(&chunk_file).unwrap();
                f.write_all(&data).unwrap();
                total_bytes += data.len() as u64;
                chunk_count += 1;
                eprintln!("[agent-sim] incremental chunk {} hash={}.. size={}", chunk_count, &hash_hex[..16], data.len());
                std::thread::sleep(Duration::from_millis(100));
            }
            "restore" => {
                let chunk_file = work_dir.join(format!("chunk_{}.bin", chunk_count % 10));
                if chunk_file.exists() {
                    let data = fs::read(&chunk_file).unwrap();
                    let hash = blake3::hash(&data);
                    let hash_hex = hex::encode(hash.as_bytes());
                    total_bytes += data.len() as u64;
                    eprintln!("[agent-sim] restore chunk {} hash={}.. size={}", chunk_count, &hash_hex[..16], data.len());
                }
                chunk_count += 1;
                std::thread::sleep(Duration::from_millis(50));
            }
            _ => {
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }

    eprintln!("[agent-sim] phase={} done: chunks={} total_bytes={}MB elapsed={}s",
        phase, chunk_count, total_bytes / 1048576, start.elapsed().as_secs());
}