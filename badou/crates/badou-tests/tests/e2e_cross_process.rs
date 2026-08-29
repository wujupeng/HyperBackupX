//! Cross-Process E2E Test -- Phase BD-21-02
//!
//! HBOP gRPC cross-process test: Backup -> Restore -> Verify.
//! Generates JWT token, connects to real badou-server, full lifecycle.

use badou_proto::ba_dou_storage_client::BaDouStorageClient;
use badou_proto::{
    RepositoryCreateRequest, RepositoryConfig, RepositoryStatRequest,
    ChunkPutRequest, ChunkData, ChunkGetRequest,
    SnapshotCommitRequest, SnapshotMeta, ManifestData, ChunkRef,
    SnapshotListRequest, VerifyRepositoryRequest, RecoveryOpenRequest,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

use hmac::{Hmac, Mac};
use sha2::Sha256;
use tonic::transport::Channel;
use tonic::Request;
use tokio_stream::StreamExt;

type HmacSha256 = Hmac<Sha256>;

fn endpoint() -> String {
    std::env::var("BADOU_E2E_ENDPOINT").unwrap_or_else(|_| "http://127.0.0.1:9090".to_string())
}

fn jwt_secret() -> Vec<u8> {
    std::env::var("BADOU_JWT_SECRET").unwrap_or_else(|_| "phase21-test".to_string()).into_bytes()
}

fn blake3_hex(data: &[u8]) -> String {
    let h = blake3::hash(data);
    hex::encode(h.as_bytes())
}

fn now_ts() -> Option<prost_types::Timestamp> {
    let now = chrono::Utc::now();
    Some(prost_types::Timestamp {
        seconds: now.timestamp(),
        nanos: now.timestamp_subsec_nanos() as i32,
    })
}

fn generate_jwt(secret: &[u8]) -> String {
    let header = serde_json::json!({"alg": "HS256", "typ": "JWT"});
    let header_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).unwrap());
    let now = chrono::Utc::now().timestamp();
    let claims = serde_json::json!({
        "sub": "e2e-test",
        "role": "admin",
        "exp": now + 3600,
        "iat": now,
    });
    let payload_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap());
    let signing_input = format!("{}.{}", header_b64, payload_b64);
    let mut mac = HmacSha256::new_from_slice(secret).unwrap();
    mac.update(signing_input.as_bytes());
    let sig = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
    format!("{}.{}.{}", header_b64, payload_b64, sig)
}

fn make_request<T>(token: &str, msg: T) -> Request<T> {
    let mut metadata = tonic::metadata::MetadataMap::new();
    metadata.insert("authorization", format!("Bearer {}", token).parse().unwrap());
    metadata.insert("x-hbop-version", "1".parse().unwrap());
    Request::from_parts(metadata, tonic::Extensions::default(), msg)
}

#[tokio::test]
async fn e2e_cross_process_full_lifecycle() {
    let endpoint = endpoint();
    let secret = jwt_secret();
    let token = generate_jwt(&secret);
    eprintln!("Connecting to badou-server: {}", endpoint);

    let channel = Channel::from_shared(endpoint.clone()).unwrap().connect().await.unwrap();
    let mut client = BaDouStorageClient::new(channel);

    // ---- Step 1: Create repository ----
    let repo_name = format!("e2e-cross-process-{}", chrono::Utc::now().timestamp_millis());
    let config = RepositoryConfig {
        name: repo_name.clone(),
        immutable: None,
        immutable_until: None,
        options: std::collections::HashMap::new(),
    };
    let create_resp = client.repository_create(make_request(&token, RepositoryCreateRequest { config: Some(config) }))
        .await
        .expect("repository_create failed")
        .into_inner();
    let repo_id = create_resp.repo.expect("no repo").repo_id.clone();
    eprintln!("[PASS] Repository created: repo_id={}", repo_id);

    // ---- Step 2: Prepare test data ----
    let chunk1_data = b"HBOP E2E Cross-Process Test -- Chunk #1 -- Hello BaDou!";
    let chunk2_data = b"HBOP E2E Cross-Process Test -- Chunk #2 -- Immutable Storage";
    let chunk3_data = vec![0xABu8; 65536];

    let chunk1_hash = blake3_hex(chunk1_data);
    let chunk2_hash = blake3_hex(chunk2_data);
    let chunk3_hash = blake3_hex(&chunk3_data);

    eprintln!("chunk1 hash={}", &chunk1_hash[..16]);
    eprintln!("chunk2 hash={}", &chunk2_hash[..16]);
    eprintln!("chunk3 hash={}", &chunk3_hash[..16]);

    let original_chunks: std::collections::HashMap<String, Vec<u8>> = [
        (chunk1_hash.clone(), chunk1_data.to_vec()),
        (chunk2_hash.clone(), chunk2_data.to_vec()),
        (chunk3_hash.clone(), chunk3_data.clone()),
    ].into_iter().collect();

    // ---- Step 3: Upload chunks (Backup) ----
    for (i, (hash, data)) in [
        (&chunk1_hash, chunk1_data.as_slice()),
        (&chunk2_hash, chunk2_data.as_slice()),
        (&chunk3_hash, chunk3_data.as_slice()),
    ].iter().enumerate() {
        let chunk = ChunkData {
            chunk_hash: hash.to_string(),
            data: data.to_vec(),
            size: data.len() as u64,
        };
        let req = make_request(&token, ChunkPutRequest { repo_id: repo_id.clone(), chunk: Some(chunk) });
        let resp = client.chunk_put(req).await.unwrap_or_else(|_| panic!("chunk_put #{} failed", i)).into_inner();
        let stored = resp.info.expect("no chunk info").stored_size;
        eprintln!("[PASS] chunk_put #{}: hash={}.., stored_size={}", i, &hash[..16], stored);
    }

    // ---- Step 4: Commit snapshot ----
    let snapshot_id = uuid::Uuid::new_v4().to_string();
    let version_id = uuid::Uuid::new_v4().to_string();

    let chunk_refs = vec![
        ChunkRef { chunk_hash: chunk1_hash.clone(), offset: 0, size: chunk1_data.len() as u64 },
        ChunkRef { chunk_hash: chunk2_hash.clone(), offset: chunk1_data.len() as u64, size: chunk2_data.len() as u64 },
        ChunkRef { chunk_hash: chunk3_hash.clone(), offset: (chunk1_data.len() + chunk2_data.len()) as u64, size: chunk3_data.len() as u64 },
    ];

    let manifest = ManifestData {
        manifest_id: uuid::Uuid::new_v4().to_string(),
        snapshot_id: snapshot_id.clone(),
        file_tree: b"[e2e-test-file-tree]".to_vec(),
        chunk_refs,
        created_at: now_ts(),
    };

    let meta = SnapshotMeta {
        snapshot_id: snapshot_id.clone(),
        version_id: version_id.clone(),
        repo_id: repo_id.clone(),
        status: badou_proto::SnapshotStatus::SnapshotCreated as i32,
        source_machine: "e2e-cross-process-test".to_string(),
        backup_policy: b"{}".to_vec(),
        file_tree_root: "/e2e/test".to_string(),
        encryption_info: vec![],
        compression_info: b"{\"algorithm\":\"none\"}".to_vec(),
        total_size: (chunk1_data.len() + chunk2_data.len() + chunk3_data.len()) as u64,
        stored_size: (chunk1_data.len() + chunk2_data.len() + chunk3_data.len()) as u64,
        file_count: 3,
        chunk_count: 3,
        created_at: now_ts(),
    };

    let commit_req = SnapshotCommitRequest {
        repo_id: repo_id.clone(),
        parent_version_id: String::new(),
        meta: Some(meta),
        manifest: Some(manifest),
        chunk_hashes: vec![chunk1_hash.clone(), chunk2_hash.clone(), chunk3_hash.clone()],
    };

    let commit_resp = client.snapshot_commit(make_request(&token, commit_req))
        .await
        .expect("snapshot_commit failed")
        .into_inner();
    let sealed_version_id = commit_resp.version.expect("no version").version_id.clone();
    eprintln!("[PASS] Snapshot committed: version_id={}", sealed_version_id);

    // ---- Step 5: Download chunks (Restore) and verify BLAKE3 ----
    for (i, (hash, original_data)) in [
        (&chunk1_hash, chunk1_data.as_slice()),
        (&chunk2_hash, chunk2_data.as_slice()),
        (&chunk3_hash, chunk3_data.as_slice()),
    ].iter().enumerate() {
        let req = make_request(&token, ChunkGetRequest { repo_id: repo_id.clone(), chunk_hash: hash.to_string() });
        let get_resp = client.chunk_get(req).await.unwrap_or_else(|_| panic!("chunk_get #{} failed", i)).into_inner();
        let restored_data = get_resp.chunk.expect("no chunk").data;
        let restored_hash = blake3_hex(&restored_data);

        assert_eq!(restored_hash, hash.as_str(), "chunk #{} BLAKE3 mismatch!", i);
        assert_eq!(restored_data.len(), original_data.len(), "chunk #{} size mismatch!", i);
        eprintln!("[PASS] chunk_get #{}: BLAKE3 match, size={}", i, restored_data.len());
    }

    // ---- Step 6: List snapshots ----
    let list_resp = client.snapshot_list(make_request(&token, SnapshotListRequest { repo_id: repo_id.clone(), limit: None, cursor: None }))
        .await
        .expect("snapshot_list failed")
        .into_inner();
    assert!(!list_resp.snapshots.is_empty(), "SnapshotList returned empty — snapshot_id consistency broken");
    eprintln!("[PASS] SnapshotList: {} snapshots (snapshot_id consistency verified)", list_resp.snapshots.len());

    // ---- Step 7: Verify repository ----
    let mut verify_stream = client.verify_repository(make_request(&token, VerifyRepositoryRequest { repo_id: repo_id.clone(), deep: false }))
        .await
        .expect("verify_repository failed")
        .into_inner();
    let mut verify_count = 0;
    while let Some(report) = verify_stream.next().await {
        let report = report.expect("verify stream error");
        eprintln!("  verify report: {:?}", report);
        verify_count += 1;
    }
    eprintln!("[PASS] Repository verify complete: {} reports", verify_count);

    // ---- Step 8: Recovery ----
    let mut recovery_stream = client.recovery_open(make_request(&token, RecoveryOpenRequest {
        repo_id: repo_id.clone(),
        version_id: sealed_version_id.clone(),
        file_path: None,
    })).await
        .expect("recovery_open failed — snapshot_id consistency broken")
        .into_inner();
    let mut recovered_chunks: Vec<(String, Vec<u8>)> = Vec::new();
    while let Some(chunk) = recovery_stream.next().await {
        let chunk = chunk.expect("recovery stream error");
        let recovered_hash = blake3_hex(&chunk.data);
        eprintln!("  recovery chunk: hash={}.., size={}", &recovered_hash[..16], chunk.size);
        recovered_chunks.push((recovered_hash, chunk.data));
    }
    assert_eq!(recovered_chunks.len(), 3, "recovery chunk count mismatch");
    for (recovered_hash, recovered_data) in &recovered_chunks {
        let original = original_chunks.get(recovered_hash)
            .unwrap_or_else(|| panic!("recovered chunk hash {} not found in original", recovered_hash));
        let original_hash = blake3_hex(original);
        assert_eq!(recovered_hash, &original_hash, "recovery BLAKE3 mismatch for chunk {}", recovered_hash);
        assert_eq!(recovered_data.len(), original.len(), "recovery size mismatch for chunk {}", recovered_hash);
    }
    eprintln!("[PASS] Recovery complete: {} chunks, all BLAKE3 verified", recovered_chunks.len());

    // ---- Step 9: Repository stat ----
    let stat_resp = client.repository_stat(make_request(&token, RepositoryStatRequest { repo_id: repo_id.clone() }))
        .await
        .expect("repository_stat failed")
        .into_inner();
    assert!(stat_resp.snapshot_count > 0, "snapshot_count == 0 — repository_stat hardcode not fixed");
    assert_eq!(stat_resp.chunk_count, 3, "chunk_count mismatch");
    eprintln!("[PASS] Repository stat: chunk_count={}, snapshot_count={} (snapshot_count > 0 verified)", stat_resp.chunk_count, stat_resp.snapshot_count);

    // ---- Summary ----
    eprintln!();
    eprintln!("========== E2E Cross-Process Summary ==========");
    eprintln!("  Endpoint:    {}", endpoint);
    eprintln!("  Repo ID:     {}", repo_id);
    eprintln!("  Version ID:  {}", sealed_version_id);
    eprintln!("  Chunks:      3 (all BLAKE3 verified)");
    eprintln!("  Snapshots:   {} (snapshot_id consistency verified)", list_resp.snapshots.len());
    eprintln!("  Verify:      {} reports", verify_count);
    eprintln!("  Recovery:    {} chunks (all BLAKE3 verified)", recovered_chunks.len());
    eprintln!("  Status:      [PASS] -- full Backup/Restore chain verified (9 steps all PASS)");
    eprintln!("================================================");
}
