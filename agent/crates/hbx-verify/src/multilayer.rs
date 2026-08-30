use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerStatus {
    Pass,
    Fail,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationLayer {
    L1SHA256,
    L2BLAKE3,
    L3Metadata,
    L4DirStructure,
    L5FileCount,
}

impl VerificationLayer {
    pub fn name(&self) -> &'static str {
        match self {
            VerificationLayer::L1SHA256 => "L1_SHA256",
            VerificationLayer::L2BLAKE3 => "L2_BLAKE3",
            VerificationLayer::L3Metadata => "L3_Metadata",
            VerificationLayer::L4DirStructure => "L4_DirStructure",
            VerificationLayer::L5FileCount => "L5_FileCount",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub path: String,
    pub size: u64,
    pub mtime_secs: i64,
    pub permissions: u32,
}

impl FileMetadata {
    pub fn new(path: String, size: u64, mtime_secs: i64, permissions: u32) -> Self {
        Self { path, size, mtime_secs, permissions }
    }
}

#[derive(Debug, Clone)]
pub struct FileVerificationResult {
    pub path: String,
    pub sha256_status: LayerStatus,
    pub blake3_status: LayerStatus,
    pub metadata_status: LayerStatus,
    pub failed_layer: Option<VerificationLayer>,
    pub detail: String,
}

impl FileVerificationResult {
    pub fn is_pass(&self) -> bool {
        self.failed_layer.is_none()
    }
}

#[derive(Debug, Clone)]
pub struct MultiLayerReport {
    pub files: Vec<FileVerificationResult>,
    pub dir_structure_status: LayerStatus,
    pub file_count_status: LayerStatus,
    pub expected_file_count: usize,
    pub actual_file_count: usize,
    pub all_passed: bool,
    pub failed_count: usize,
    pub failed_layers: Vec<VerificationLayer>,
}

impl MultiLayerReport {
    pub fn passed_count(&self) -> usize {
        self.files.iter().filter(|f| f.is_pass()).count()
    }
}

pub struct MultiLayerVerifier;

impl MultiLayerVerifier {
    pub fn new() -> Self {
        Self
    }

    pub fn verify(
        &self,
        expected_files: &[FileMetadata],
        actual_files: &[FileMetadata],
        expected_sha256: &HashMap<String, [u8; 32]>,
        actual_sha256: &HashMap<String, [u8; 32]>,
        expected_blake3: &HashMap<String, [u8; 32]>,
        actual_blake3: &HashMap<String, [u8; 32]>,
    ) -> MultiLayerReport {
        let mut file_results = Vec::with_capacity(expected_files.len());
        let mut failed_layers = Vec::new();

        let actual_by_path: HashMap<&str, &FileMetadata> = actual_files
            .iter()
            .map(|f| (f.path.as_str(), f))
            .collect();

        for expected in expected_files {
            let result = self.verify_file(
                expected,
                actual_by_path.get(expected.path.as_str()).copied(),
                expected_sha256,
                actual_sha256,
                expected_blake3,
                actual_blake3,
            );
            if let Some(ref layer) = result.failed_layer {
                if !failed_layers.contains(layer) {
                    failed_layers.push(layer.clone());
                }
            }
            file_results.push(result);
        }

        let expected_paths: Vec<&str> = expected_files.iter().map(|f| f.path.as_str()).collect();
        let actual_paths: Vec<&str> = actual_files.iter().map(|f| f.path.as_str()).collect();
        let dir_structure_status = self.verify_dir_structure(&expected_paths, &actual_paths);
        if dir_structure_status == LayerStatus::Fail && !failed_layers.contains(&VerificationLayer::L4DirStructure) {
            failed_layers.push(VerificationLayer::L4DirStructure);
        }

        let file_count_status = self.verify_file_count(expected_files.len(), actual_files.len());
        if file_count_status == LayerStatus::Fail && !failed_layers.contains(&VerificationLayer::L5FileCount) {
            failed_layers.push(VerificationLayer::L5FileCount);
        }

        let failed_count = file_results.iter().filter(|f| !f.is_pass()).count();
        let all_passed = failed_count == 0
            && dir_structure_status == LayerStatus::Pass
            && file_count_status == LayerStatus::Pass;

        MultiLayerReport {
            files: file_results,
            dir_structure_status,
            file_count_status,
            expected_file_count: expected_files.len(),
            actual_file_count: actual_files.len(),
            all_passed,
            failed_count,
            failed_layers,
        }
    }

    fn verify_file(
        &self,
        expected: &FileMetadata,
        actual: Option<&FileMetadata>,
        expected_sha256: &HashMap<String, [u8; 32]>,
        actual_sha256: &HashMap<String, [u8; 32]>,
        expected_blake3: &HashMap<String, [u8; 32]>,
        actual_blake3: &HashMap<String, [u8; 32]>,
    ) -> FileVerificationResult {
        let mut result = FileVerificationResult {
            path: expected.path.clone(),
            sha256_status: LayerStatus::Pass,
            blake3_status: LayerStatus::Pass,
            metadata_status: LayerStatus::Pass,
            failed_layer: None,
            detail: String::new(),
        };

        let actual = match actual {
            Some(a) => a,
            None => {
                result.sha256_status = LayerStatus::Fail;
                result.blake3_status = LayerStatus::Skipped;
                result.metadata_status = LayerStatus::Skipped;
                result.failed_layer = Some(VerificationLayer::L1SHA256);
                result.detail = format!("file not found in actual: {}", expected.path);
                return result;
            }
        };

        let expected_hash = expected_sha256.get(&expected.path);
        let actual_hash = actual_sha256.get(&actual.path);
        match (expected_hash, actual_hash) {
            (Some(eh), Some(ah)) if eh == ah => {
                result.sha256_status = LayerStatus::Pass;
            }
            _ => {
                result.sha256_status = LayerStatus::Fail;
                result.blake3_status = LayerStatus::Skipped;
                result.metadata_status = LayerStatus::Skipped;
                result.failed_layer = Some(VerificationLayer::L1SHA256);
                result.detail = format!("SHA-256 mismatch for {}", expected.path);
                return result;
            }
        }

        let expected_b3 = expected_blake3.get(&expected.path);
        let actual_b3 = actual_blake3.get(&actual.path);
        match (expected_b3, actual_b3) {
            (Some(eh), Some(ah)) if eh == ah => {
                result.blake3_status = LayerStatus::Pass;
            }
            _ => {
                result.blake3_status = LayerStatus::Fail;
                result.metadata_status = LayerStatus::Skipped;
                result.failed_layer = Some(VerificationLayer::L2BLAKE3);
                result.detail = format!("BLAKE3 mismatch for {}", expected.path);
                return result;
            }
        }

        if expected.size != actual.size {
            result.metadata_status = LayerStatus::Fail;
            result.failed_layer = Some(VerificationLayer::L3Metadata);
            result.detail = format!(
                "size mismatch for {}: expected={}, actual={}",
                expected.path, expected.size, actual.size
            );
            return result;
        }

        if expected.mtime_secs != actual.mtime_secs {
            result.metadata_status = LayerStatus::Fail;
            result.failed_layer = Some(VerificationLayer::L3Metadata);
            result.detail = format!(
                "mtime mismatch for {}: expected={}, actual={}",
                expected.path, expected.mtime_secs, actual.mtime_secs
            );
            return result;
        }

        if expected.permissions != actual.permissions {
            result.metadata_status = LayerStatus::Fail;
            result.failed_layer = Some(VerificationLayer::L3Metadata);
            result.detail = format!(
                "permissions mismatch for {}: expected={}, actual={}",
                expected.path, expected.permissions, actual.permissions
            );
            return result;
        }

        result.detail = format!("all layers passed for {}", expected.path);
        result
    }

    fn verify_dir_structure(&self, expected_paths: &[&str], actual_paths: &[&str]) -> LayerStatus {
        let expected_set: std::collections::HashSet<&str> = expected_paths.iter().copied().collect();
        let actual_set: std::collections::HashSet<&str> = actual_paths.iter().copied().collect();

        if expected_set == actual_set {
            LayerStatus::Pass
        } else {
            LayerStatus::Fail
        }
    }

    fn verify_file_count(&self, expected: usize, actual: usize) -> LayerStatus {
        if expected == actual {
            LayerStatus::Pass
        } else {
            LayerStatus::Fail
        }
    }

    pub fn verify_directories(
        &self,
        expected_dirs: &[PathBuf],
        actual_dirs: &[PathBuf],
    ) -> LayerStatus {
        let expected_set: std::collections::HashSet<&PathBuf> = expected_dirs.iter().collect();
        let actual_set: std::collections::HashSet<&PathBuf> = actual_dirs.iter().collect();

        if expected_set == actual_set {
            LayerStatus::Pass
        } else {
            LayerStatus::Fail
        }
    }
}

impl Default for MultiLayerVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_metadata(path: &str, size: u64, mtime: i64, perms: u32) -> FileMetadata {
        FileMetadata::new(path.to_string(), size, mtime, perms)
    }

    fn make_hashes(paths: &[&str], seed: u8) -> HashMap<String, [u8; 32]> {
        let mut map = HashMap::new();
        for path in paths {
            let mut hash = [0u8; 32];
            hash[0] = seed;
            hash[31] = seed;
            map.insert(path.to_string(), hash);
        }
        map
    }

    #[test]
    fn test_all_layers_pass() {
        let verifier = MultiLayerVerifier::new();
        let expected = vec![make_metadata("a.txt", 100, 1000, 0o644)];
        let actual = vec![make_metadata("a.txt", 100, 1000, 0o644)];
        let sha256 = make_hashes(&["a.txt"], 1);
        let blake3 = make_hashes(&["a.txt"], 2);

        let report = verifier.verify(&expected, &actual, &sha256, &sha256, &blake3, &blake3);

        assert!(report.all_passed);
        assert_eq!(report.failed_count, 0);
        assert_eq!(report.dir_structure_status, LayerStatus::Pass);
        assert_eq!(report.file_count_status, LayerStatus::Pass);
        assert!(report.failed_layers.is_empty());
    }

    #[test]
    fn test_sha256_mismatch_stops_subsequent() {
        let verifier = MultiLayerVerifier::new();
        let expected = vec![make_metadata("a.txt", 100, 1000, 0o644)];
        let actual = vec![make_metadata("a.txt", 100, 1000, 0o644)];
        let expected_sha = make_hashes(&["a.txt"], 1);
        let actual_sha = make_hashes(&["a.txt"], 2);
        let blake3 = make_hashes(&["a.txt"], 3);

        let report = verifier.verify(&expected, &actual, &expected_sha, &actual_sha, &blake3, &blake3);

        assert!(!report.all_passed);
        assert_eq!(report.failed_count, 1);
        assert_eq!(report.files[0].sha256_status, LayerStatus::Fail);
        assert_eq!(report.files[0].blake3_status, LayerStatus::Skipped);
        assert_eq!(report.files[0].metadata_status, LayerStatus::Skipped);
        assert_eq!(report.files[0].failed_layer, Some(VerificationLayer::L1SHA256));
    }

    #[test]
    fn test_blake3_mismatch_stops_metadata() {
        let verifier = MultiLayerVerifier::new();
        let expected = vec![make_metadata("a.txt", 100, 1000, 0o644)];
        let actual = vec![make_metadata("a.txt", 100, 1000, 0o644)];
        let sha = make_hashes(&["a.txt"], 1);
        let expected_b3 = make_hashes(&["a.txt"], 2);
        let actual_b3 = make_hashes(&["a.txt"], 3);

        let report = verifier.verify(&expected, &actual, &sha, &sha, &expected_b3, &actual_b3);

        assert!(!report.all_passed);
        assert_eq!(report.files[0].sha256_status, LayerStatus::Pass);
        assert_eq!(report.files[0].blake3_status, LayerStatus::Fail);
        assert_eq!(report.files[0].metadata_status, LayerStatus::Skipped);
        assert_eq!(report.files[0].failed_layer, Some(VerificationLayer::L2BLAKE3));
    }

    #[test]
    fn test_size_mismatch() {
        let verifier = MultiLayerVerifier::new();
        let expected = vec![make_metadata("a.txt", 100, 1000, 0o644)];
        let actual = vec![make_metadata("a.txt", 200, 1000, 0o644)];
        let sha = make_hashes(&["a.txt"], 1);
        let blake3 = make_hashes(&["a.txt"], 2);

        let report = verifier.verify(&expected, &actual, &sha, &sha, &blake3, &blake3);

        assert!(!report.all_passed);
        assert_eq!(report.files[0].metadata_status, LayerStatus::Fail);
        assert_eq!(report.files[0].failed_layer, Some(VerificationLayer::L3Metadata));
    }

    #[test]
    fn test_mtime_mismatch() {
        let verifier = MultiLayerVerifier::new();
        let expected = vec![make_metadata("a.txt", 100, 1000, 0o644)];
        let actual = vec![make_metadata("a.txt", 100, 2000, 0o644)];
        let sha = make_hashes(&["a.txt"], 1);
        let blake3 = make_hashes(&["a.txt"], 2);

        let report = verifier.verify(&expected, &actual, &sha, &sha, &blake3, &blake3);

        assert!(!report.all_passed);
        assert_eq!(report.files[0].failed_layer, Some(VerificationLayer::L3Metadata));
    }

    #[test]
    fn test_permissions_mismatch() {
        let verifier = MultiLayerVerifier::new();
        let expected = vec![make_metadata("a.txt", 100, 1000, 0o644)];
        let actual = vec![make_metadata("a.txt", 100, 1000, 0o755)];
        let sha = make_hashes(&["a.txt"], 1);
        let blake3 = make_hashes(&["a.txt"], 2);

        let report = verifier.verify(&expected, &actual, &sha, &sha, &blake3, &blake3);

        assert!(!report.all_passed);
        assert_eq!(report.files[0].failed_layer, Some(VerificationLayer::L3Metadata));
    }

    #[test]
    fn test_file_not_found_in_actual() {
        let verifier = MultiLayerVerifier::new();
        let expected = vec![make_metadata("a.txt", 100, 1000, 0o644)];
        let actual: Vec<FileMetadata> = vec![];
        let sha = make_hashes(&["a.txt"], 1);
        let blake3 = make_hashes(&["a.txt"], 2);

        let report = verifier.verify(&expected, &actual, &sha, &sha, &blake3, &blake3);

        assert!(!report.all_passed);
        assert_eq!(report.files[0].failed_layer, Some(VerificationLayer::L1SHA256));
        assert_eq!(report.dir_structure_status, LayerStatus::Fail);
        assert_eq!(report.file_count_status, LayerStatus::Fail);
    }

    #[test]
    fn test_dir_structure_mismatch() {
        let verifier = MultiLayerVerifier::new();
        let expected = vec![
            make_metadata("a.txt", 100, 1000, 0o644),
            make_metadata("b.txt", 200, 2000, 0o644),
        ];
        let actual = vec![make_metadata("a.txt", 100, 1000, 0o644)];
        let sha = make_hashes(&["a.txt", "b.txt"], 1);
        let blake3 = make_hashes(&["a.txt", "b.txt"], 2);

        let report = verifier.verify(&expected, &actual, &sha, &sha, &blake3, &blake3);

        assert!(!report.all_passed);
        assert_eq!(report.dir_structure_status, LayerStatus::Fail);
        assert_eq!(report.file_count_status, LayerStatus::Fail);
    }

    #[test]
    fn test_file_count_mismatch() {
        let verifier = MultiLayerVerifier::new();
        let expected = vec![make_metadata("a.txt", 100, 1000, 0o644)];
        let actual = vec![
            make_metadata("a.txt", 100, 1000, 0o644),
            make_metadata("b.txt", 200, 2000, 0o644),
        ];
        let sha = make_hashes(&["a.txt"], 1);
        let blake3 = make_hashes(&["a.txt"], 2);

        let report = verifier.verify(&expected, &actual, &sha, &sha, &blake3, &blake3);

        assert!(!report.all_passed);
        assert_eq!(report.file_count_status, LayerStatus::Fail);
    }

    #[test]
    fn test_multiple_files_all_pass() {
        let verifier = MultiLayerVerifier::new();
        let expected = vec![
            make_metadata("a.txt", 100, 1000, 0o644),
            make_metadata("b.txt", 200, 2000, 0o644),
            make_metadata("c.txt", 300, 3000, 0o755),
        ];
        let actual = expected.clone();
        let sha = make_hashes(&["a.txt", "b.txt", "c.txt"], 1);
        let blake3 = make_hashes(&["a.txt", "b.txt", "c.txt"], 2);

        let report = verifier.verify(&expected, &actual, &sha, &sha, &blake3, &blake3);

        assert!(report.all_passed);
        assert_eq!(report.passed_count(), 3);
    }

    #[test]
    fn test_empty_files_all_pass() {
        let verifier = MultiLayerVerifier::new();
        let report = verifier.verify(&[], &[], &HashMap::new(), &HashMap::new(), &HashMap::new(), &HashMap::new());

        assert!(report.all_passed);
        assert_eq!(report.expected_file_count, 0);
        assert_eq!(report.actual_file_count, 0);
    }

    #[test]
    fn test_failed_layers_collected() {
        let verifier = MultiLayerVerifier::new();
        let expected = vec![
            make_metadata("a.txt", 100, 1000, 0o644),
            make_metadata("b.txt", 200, 2000, 0o644),
        ];
        let actual = vec![make_metadata("a.txt", 100, 1000, 0o644)];
        let sha = make_hashes(&["a.txt", "b.txt"], 1);
        let blake3 = make_hashes(&["a.txt", "b.txt"], 2);

        let report = verifier.verify(&expected, &actual, &sha, &sha, &blake3, &blake3);

        assert!(report.failed_layers.contains(&VerificationLayer::L1SHA256));
        assert!(report.failed_layers.contains(&VerificationLayer::L4DirStructure));
        assert!(report.failed_layers.contains(&VerificationLayer::L5FileCount));
    }

    #[test]
    fn test_layer_name() {
        assert_eq!(VerificationLayer::L1SHA256.name(), "L1_SHA256");
        assert_eq!(VerificationLayer::L2BLAKE3.name(), "L2_BLAKE3");
        assert_eq!(VerificationLayer::L3Metadata.name(), "L3_Metadata");
        assert_eq!(VerificationLayer::L4DirStructure.name(), "L4_DirStructure");
        assert_eq!(VerificationLayer::L5FileCount.name(), "L5_FileCount");
    }
}