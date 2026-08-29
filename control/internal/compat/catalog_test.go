package compat

import "testing"

func TestCatalogContainsAllCoreFeatures(t *testing.T) {
	catalog := NewDuplicatiFeatureCatalog()

	requiredFeatures := []string{
		"full_backup", "incremental_backup", "block_level_incremental", "forever_incremental",
		"full_restore", "selective_restore", "restore_overwrite_policy",
		"block_dedup", "global_dedup",
		"zstd_compression", "lz4_compression",
		"aes256_encryption", "gpg_encryption",
		"keep_all_versions", "keep_number_of_versions", "smart_retention",
		"include_filter", "exclude_filter", "glob_filter", "regex_filter",
		"locked_file_handling", "resume_interrupted_backup", "bandwidth_throttle",
		"large_file_support", "unicode_filenames", "long_path_support",
		"cli_interface", "web_ui", "api_interface",
		"config_import", "config_export",
		"file_metadata", "hardlink_support", "symlink_support", "acl_support",
	}

	for _, name := range requiredFeatures {
		found := false
		for _, f := range catalog.Features {
			if f.Name == name {
				found = true
				break
			}
		}
		if !found {
			t.Errorf("required feature %q not found in catalog", name)
		}
	}
}

func TestCatalogFeatureCount(t *testing.T) {
	catalog := NewDuplicatiFeatureCatalog()
	if catalog.Count() < 40 {
		t.Errorf("expected at least 40 features, got %d", catalog.Count())
	}
}

func TestCatalogGetByCategory(t *testing.T) {
	catalog := NewDuplicatiFeatureCatalog()

	backupFeatures := catalog.GetByCategory(CategoryBackup)
	if len(backupFeatures) == 0 {
		t.Error("expected backup features, got none")
	}

	restoreFeatures := catalog.GetByCategory(CategoryRestore)
	if len(restoreFeatures) == 0 {
		t.Error("expected restore features, got none")
	}
}

func TestCatalogFind(t *testing.T) {
	catalog := NewDuplicatiFeatureCatalog()

	f, ok := catalog.Find("F001")
	if !ok {
		t.Error("expected to find feature F001")
	}
	if f.Name != "full_backup" {
		t.Errorf("expected feature name 'full_backup', got %q", f.Name)
	}

	_, ok = catalog.Find("NONEXIST")
	if ok {
		t.Error("expected not to find feature NONEXIST")
	}
}

func TestAllFeaturesHaveUniqueIDs(t *testing.T) {
	catalog := NewDuplicatiFeatureCatalog()
	seen := make(map[string]bool)
	for _, f := range catalog.Features {
		if seen[f.FeatureID] {
			t.Errorf("duplicate feature ID: %s", f.FeatureID)
		}
		seen[f.FeatureID] = true
	}
}