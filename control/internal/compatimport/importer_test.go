package compatimport

import (
	"encoding/json"
	"testing"
)

func makeConfigJSON(name string, sources []string) []byte {
	cfg := map[string]interface{}{
		"Name":    name,
		"Sources": sources,
		"Destination": map[string]interface{}{
			"Type": "file",
			"Path": "/backup/dest",
		},
		"Encryption": map[string]interface{}{
			"Type": "AES256",
		},
		"Compression": "zip",
	}
	data, _ := json.Marshal(cfg)
	return data
}

func TestParseJSONConfig(t *testing.T) {
	imp := NewImporter(UnsupportedSkip)
	data := makeConfigJSON("test-backup", []string{"/data/source"})
	cfg, err := imp.ParseConfig(FormatJSON, data)
	if err != nil {
		t.Fatalf("parse failed: %v", err)
	}
	if cfg.Name != "test-backup" {
		t.Errorf("expected name 'test-backup', got %s", cfg.Name)
	}
	if len(cfg.Sources) != 1 || cfg.Sources[0] != "/data/source" {
		t.Errorf("sources mismatch")
	}
}

func TestParseJSONConfigMissingName(t *testing.T) {
	imp := NewImporter(UnsupportedSkip)
	data := []byte(`{"Sources": ["/data"]}`)
	_, err := imp.ParseConfig(FormatJSON, data)
	if err == nil {
		t.Error("expected error for missing Name")
	}
}

func TestParseJSONConfigMissingSources(t *testing.T) {
	imp := NewImporter(UnsupportedSkip)
	data := []byte(`{"Name": "test"}`)
	_, err := imp.ParseConfig(FormatJSON, data)
	if err == nil {
		t.Error("expected error for missing Sources")
	}
}

func TestParseSQLiteNotImplemented(t *testing.T) {
	imp := NewImporter(UnsupportedSkip)
	_, err := imp.ParseConfig(FormatSQLite, []byte("data"))
	if err == nil {
		t.Error("expected error for sqlite format")
	}
}

func TestParseUnknownFormat(t *testing.T) {
	imp := NewImporter(UnsupportedSkip)
	_, err := imp.ParseConfig("unknown", []byte("data"))
	if err == nil {
		t.Error("expected error for unknown format")
	}
}

func TestMapFieldsBasic(t *testing.T) {
	imp := NewImporter(UnsupportedSkip)
	cfg := &DuplicatiConfig{
		Name:    "test-backup",
		Sources: []string{"/data"},
		Destination: map[string]interface{}{"Type": "file", "Path": "/backup"},
		Encryption:  map[string]interface{}{"Type": "AES256"},
		Compression: "zip",
	}
	mappings, unsupported := imp.MapFields(cfg)
	if len(unsupported) != 0 {
		t.Errorf("expected no unsupported items, got %d", len(unsupported))
	}
	foundName := false
	for _, m := range mappings {
		if m.HBXField == "job.name" && m.HBXValue == "test-backup" {
			foundName = true
		}
	}
	if !foundName {
		t.Error("name mapping not found")
	}
}

func TestMapFieldsEncryptionMapping(t *testing.T) {
	imp := NewImporter(UnsupportedSkip)
	cfg := &DuplicatiConfig{
		Name:       "test",
		Sources:    []string{"/data"},
		Encryption: map[string]interface{}{"Type": "AES256"},
	}
	mappings, _ := imp.MapFields(cfg)
	for _, m := range mappings {
		if m.DuplicatiField == "Encryption.Type" {
			if m.HBXValue != "aes-256-gcm" {
				t.Errorf("expected aes-256-gcm, got %v", m.HBXValue)
			}
			return
		}
	}
	t.Error("encryption mapping not found")
}

func TestMapFieldsUnsupportedOption(t *testing.T) {
	imp := NewImporter(UnsupportedSkip)
	cfg := &DuplicatiConfig{
		Name:    "test",
		Sources: []string{"/data"},
		Options: map[string]interface{}{
			"blocksize":          "100kb",
			"native-tail-reading": true,
		},
	}
	_, unsupported := imp.MapFields(cfg)
	if len(unsupported) != 1 {
		t.Errorf("expected 1 unsupported item, got %d", len(unsupported))
	}
	if unsupported[0].Field != "Options.native-tail-reading" {
		t.Errorf("unexpected unsupported field: %s", unsupported[0].Field)
	}
}

func TestMapFieldsUnsupportedFilter(t *testing.T) {
	imp := NewImporter(UnsupportedSkip)
	cfg := &DuplicatiConfig{
		Name:    "test",
		Sources: []string{"/data"},
		Filters: []map[string]interface{}{
			{"Type": "include", "Match": "/data"},
			{"Type": "extension-list", "Match": "tmp,log"},
		},
	}
	_, unsupported := imp.MapFields(cfg)
	found := false
	for _, u := range unsupported {
		if u.Field == "Filters" {
			found = true
		}
	}
	if !found {
		t.Error("unsupported filter not reported")
	}
}

func TestImportSuccess(t *testing.T) {
	imp := NewImporter(UnsupportedSkip)
	data := makeConfigJSON("test-backup", []string{"/data"})
	result, err := imp.Import(FormatJSON, data)
	if err != nil {
		t.Fatalf("import failed: %v", err)
	}
	if result.Status != ImportSuccess {
		t.Errorf("expected success, got %s", result.Status)
	}
	if result.ResultingJobID == nil {
		t.Error("expected non-nil job ID")
	}
	if result.Idempotent {
		t.Error("first import should not be idempotent")
	}
}

func TestImportIdempotent(t *testing.T) {
	imp := NewImporter(UnsupportedSkip)
	data := makeConfigJSON("test-backup", []string{"/data"})
	result1, _ := imp.Import(FormatJSON, data)
	result2, _ := imp.Import(FormatJSON, data)
	if !result2.Idempotent {
		t.Error("second import should be idempotent")
	}
	if result1.ImportID != result2.ImportID {
		t.Error("idempotent import should return same import ID")
	}
}

func TestImportWithUnsupportedSkip(t *testing.T) {
	imp := NewImporter(UnsupportedSkip)
	cfg := map[string]interface{}{
		"Name":    "test",
		"Sources": []string{"/data"},
		"Options": map[string]interface{}{
			"native-tail-reading": true,
		},
	}
	data, _ := json.Marshal(cfg)
	result, err := imp.Import(FormatJSON, data)
	if err != nil {
		t.Fatalf("import failed: %v", err)
	}
	if result.Status != ImportPartial {
		t.Errorf("expected partial, got %s", result.Status)
	}
	if result.ResultingJobID == nil {
		t.Error("job should still be created with skip policy")
	}
}

func TestImportWithUnsupportedAbort(t *testing.T) {
	imp := NewImporter(UnsupportedAbort)
	cfg := map[string]interface{}{
		"Name":    "test",
		"Sources": []string{"/data"},
		"Options": map[string]interface{}{
			"native-tail-reading": true,
		},
	}
	data, _ := json.Marshal(cfg)
	result, err := imp.Import(FormatJSON, data)
	if err != nil {
		t.Fatalf("import failed: %v", err)
	}
	if result.Status != ImportFailed {
		t.Errorf("expected failed, got %s", result.Status)
	}
	if result.ResultingJobID != nil {
		t.Error("job should not be created with abort policy")
	}
}

func TestImportParseError(t *testing.T) {
	imp := NewImporter(UnsupportedSkip)
	result, err := imp.Import(FormatJSON, []byte("invalid json"))
	if err == nil {
		t.Error("expected error for invalid JSON")
	}
	if result.Status != ImportFailed {
		t.Errorf("expected failed status, got %s", result.Status)
	}
}

func TestComputeHashDeterministic(t *testing.T) {
	imp := NewImporter(UnsupportedSkip)
	data := []byte("test config data")
	hash1 := imp.ComputeHash(data)
	hash2 := imp.ComputeHash(data)
	if hash1 != hash2 {
		t.Error("hash should be deterministic")
	}
}

func TestComputeHashDifferent(t *testing.T) {
	imp := NewImporter(UnsupportedSkip)
	hash1 := imp.ComputeHash([]byte("config1"))
	hash2 := imp.ComputeHash([]byte("config2"))
	if hash1 == hash2 {
		t.Error("different configs should have different hashes")
	}
}

func TestGetImportByHash(t *testing.T) {
	imp := NewImporter(UnsupportedSkip)
	data := makeConfigJSON("test", []string{"/data"})
	result, _ := imp.Import(FormatJSON, data)
	got, ok := imp.GetImportByHash(result.SourceHash)
	if !ok {
		t.Fatal("import not found by hash")
	}
	if got.ImportID != result.ImportID {
		t.Error("import ID mismatch")
	}
}

func TestListImports(t *testing.T) {
	imp := NewImporter(UnsupportedSkip)
	imp.Import(FormatJSON, makeConfigJSON("test1", []string{"/data1"}))
	imp.Import(FormatJSON, makeConfigJSON("test2", []string{"/data2"}))
	imports := imp.ListImports()
	if len(imports) != 2 {
		t.Errorf("expected 2 imports, got %d", len(imports))
	}
}

func TestMapDestType(t *testing.T) {
	tests := []struct {
		input  string
		expect string
	}{
		{"file", "local"},
		{"local", "local"},
		{"s3", "s3"},
		{"azure", "azure_blob"},
		{"gcs", "gcs"},
		{"unknown", "unknown"},
	}
	for _, tc := range tests {
		if got := mapDestType(tc.input); got != tc.expect {
			t.Errorf("mapDestType(%s) = %s, expected %s", tc.input, got, tc.expect)
		}
	}
}

func TestMapEncryptionType(t *testing.T) {
	tests := []struct {
		input  string
		expect string
	}{
		{"AES256", "aes-256-gcm"},
		{"AES-256", "aes-256-gcm"},
		{"PGP", "pgp"},
		{"none", "none"},
		{"", "none"},
	}
	for _, tc := range tests {
		if got := mapEncryptionType(tc.input); got != tc.expect {
			t.Errorf("mapEncryptionType(%s) = %s, expected %s", tc.input, got, tc.expect)
		}
	}
}

func TestMapCompression(t *testing.T) {
	tests := []struct {
		input  string
		expect string
	}{
		{"zip", "zstd"},
		{"gzip", "gzip"},
		{"none", "none"},
		{"custom", "custom"},
	}
	for _, tc := range tests {
		if got := mapCompression(tc.input); got != tc.expect {
			t.Errorf("mapCompression(%s) = %s, expected %s", tc.input, got, tc.expect)
		}
	}
}

func TestImportOriginalConfigNotModified(t *testing.T) {
	imp := NewImporter(UnsupportedSkip)
	original := makeConfigJSON("test-backup", []string{"/data"})
	originalCopy := make([]byte, len(original))
	copy(originalCopy, original)

	imp.Import(FormatJSON, original)

	for i := range original {
		if original[i] != originalCopy[i] {
			t.Error("original config data was modified during import")
			break
		}
	}
}