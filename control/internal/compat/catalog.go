package compat
package compat

type FeatureCategory string

const (
	CategoryBackup       FeatureCategory = "backup"
	CategoryRestore      FeatureCategory = "restore"
	CategoryDedup        FeatureCategory = "dedup"
	CategoryCompression  FeatureCategory = "compression"
	CategoryEncryption   FeatureCategory = "encryption"
	CategoryRetention    FeatureCategory = "retention"
	CategoryFilter       FeatureCategory = "filter"
	CategoryResilience   FeatureCategory = "resilience"
	CategoryScalability  FeatureCategory = "scalability"
	CategoryInterface    FeatureCategory = "interface"
	CategoryConfig       FeatureCategory = "config"
	CategoryMetadata     FeatureCategory = "metadata"
)

type DuplicatiFeature struct {
	FeatureID string          `json:"feature_id"`
	Name      string          `json:"name"`
	Category  FeatureCategory `json:"category"`
}

type DuplicatiFeatureCatalog struct {
	Features []DuplicatiFeature `json:"features"`
}

func NewDuplicatiFeatureCatalog() *DuplicatiFeatureCatalog {
	return &DuplicatiFeatureCatalog{
		Features: duplicatiFeatures(),
	}
}

func duplicatiFeatures() []DuplicatiFeature {
	return []DuplicatiFeature{
		{FeatureID: "F001", Name: "full_backup", Category: CategoryBackup},
		{FeatureID: "F002", Name: "incremental_backup", Category: CategoryBackup},
		{FeatureID: "F003", Name: "block_level_incremental", Category: CategoryBackup},
		{FeatureID: "F004", Name: "forever_incremental", Category: CategoryBackup},
		{FeatureID: "F005", Name: "scheduled_backup", Category: CategoryBackup},
		{FeatureID: "F006", Name: "backup_verification", Category: CategoryBackup},

		{FeatureID: "F101", Name: "full_restore", Category: CategoryRestore},
		{FeatureID: "F102", Name: "selective_restore", Category: CategoryRestore},
		{FeatureID: "F103", Name: "restore_to_original", Category: CategoryRestore},
		{FeatureID: "F104", Name: "restore_to_new_location", Category: CategoryRestore},
		{FeatureID: "F105", Name: "restore_overwrite_policy", Category: CategoryRestore},
		{FeatureID: "F106", Name: "point_in_time_restore", Category: CategoryRestore},

		{FeatureID: "F201", Name: "block_dedup", Category: CategoryDedup},
		{FeatureID: "F202", Name: "global_dedup", Category: CategoryDedup},
		{FeatureID: "F203", Name: "dedup_ratio_reporting", Category: CategoryDedup},

		{FeatureID: "F301", Name: "zstd_compression", Category: CategoryCompression},
		{FeatureID: "F302", Name: "lz4_compression", Category: CategoryCompression},
		{FeatureID: "F303", Name: "gzip_compression", Category: CategoryCompression},
		{FeatureID: "F304", Name: "no_compression", Category: CategoryCompression},
		{FeatureID: "F305", Name: "compression_level", Category: CategoryCompression},

		{FeatureID: "F401", Name: "aes256_encryption", Category: CategoryEncryption},
		{FeatureID: "F402", Name: "gpg_encryption", Category: CategoryEncryption},
		{FeatureID: "F403", Name: "no_encryption", Category: CategoryEncryption},
		{FeatureID: "F404", Name: "key_derivation", Category: CategoryEncryption},

		{FeatureID: "F501", Name: "keep_all_versions", Category: CategoryRetention},
		{FeatureID: "F502", Name: "keep_number_of_versions", Category: CategoryRetention},
		{FeatureID: "F503", Name: "keep_time_interval", Category: CategoryRetention},
		{FeatureID: "F504", Name: "smart_retention", Category: CategoryRetention},

		{FeatureID: "F601", Name: "include_filter", Category: CategoryFilter},
		{FeatureID: "F602", Name: "exclude_filter", Category: CategoryFilter},
		{FeatureID: "F603", Name: "glob_filter", Category: CategoryFilter},
		{FeatureID: "F604", Name: "regex_filter", Category: CategoryFilter},

		{FeatureID: "F701", Name: "locked_file_handling", Category: CategoryResilience},
		{FeatureID: "F702", Name: "resume_interrupted_backup", Category: CategoryResilience},
		{FeatureID: "F703", Name: "network_retry", Category: CategoryResilience},
		{FeatureID: "F704", Name: "bandwidth_throttle", Category: CategoryResilience},
		{FeatureID: "F705", Name: "backup_lock", Category: CategoryResilience},

		{FeatureID: "F801", Name: "large_file_support", Category: CategoryScalability},
		{FeatureID: "F802", Name: "unicode_filenames", Category: CategoryScalability},
		{FeatureID: "F803", Name: "long_path_support", Category: CategoryScalability},
		{FeatureID: "F804", Name: "many_files_support", Category: CategoryScalability},
		{FeatureID: "F805", Name: "multi_destination", Category: CategoryScalability},

		{FeatureID: "F901", Name: "cli_interface", Category: CategoryInterface},
		{FeatureID: "F902", Name: "web_ui", Category: CategoryInterface},
		{FeatureID: "F903", Name: "api_interface", Category: CategoryInterface},
		{FeatureID: "F904", Name: "progress_reporting", Category: CategoryInterface},
		{FeatureID: "F905", Name: "notifications", Category: CategoryInterface},

		{FeatureID: "FA01", Name: "config_import", Category: CategoryConfig},
		{FeatureID: "FA02", Name: "config_export", Category: CategoryConfig},
		{FeatureID: "FA03", Name: "command_line_export", Category: CategoryConfig},
		{FeatureID: "FA04", Name: "no_backend_secret_in_config", Category: CategoryConfig},

		{FeatureID: "FB01", Name: "file_metadata", Category: CategoryMetadata},
		{FeatureID: "FB02", Name: "hardlink_support", Category: CategoryMetadata},
		{FeatureID: "FB03", Name: "symlink_support", Category: CategoryMetadata},
		{FeatureID: "FB04", Name: "acl_support", Category: CategoryMetadata},
		{FeatureID: "FB05", Name: "xattr_support", Category: CategoryMetadata},
		{FeatureID: "FB06", Name: "timestamp_preservation", Category: CategoryMetadata},
	}
}

func (c *DuplicatiFeatureCatalog) Count() int {
	return len(c.Features)
}

func (c *DuplicatiFeatureCatalog) GetByCategory(cat FeatureCategory) []DuplicatiFeature {
	var result []DuplicatiFeature
	for _, f := range c.Features {
		if f.Category == cat {
			result = append(result, f)
		}
	}
	return result
}

func (c *DuplicatiFeatureCatalog) Find(featureID string) (DuplicatiFeature, bool) {
	for _, f := range c.Features {
		if f.FeatureID == featureID {
			return f, true
		}
	}
	return DuplicatiFeature{}, false
}