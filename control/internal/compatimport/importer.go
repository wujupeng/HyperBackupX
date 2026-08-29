package compatimport

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"sync"
	"time"

	"github.com/google/uuid"
)

type SourceFormat string

const (
	FormatJSON   SourceFormat = "json"
	FormatSQLite SourceFormat = "sqlite"
	FormatXML    SourceFormat = "xml"
)

type ImportStatus string

const (
	ImportSuccess ImportStatus = "success"
	ImportPartial ImportStatus = "partial"
	ImportFailed  ImportStatus = "failed"
)

type UnsupportedPolicy string

const (
	UnsupportedAbort  UnsupportedPolicy = "abort"
	UnsupportedSkip   UnsupportedPolicy = "skip"
)

type DuplicatiConfig struct {
	Name        string                 `json:"Name"`
	Sources     []string               `json:"Sources"`
	Destination map[string]interface{} `json:"Destination"`
	Encryption  map[string]interface{} `json:"Encryption"`
	Compression string                 `json:"Compression"`
	Schedule    map[string]interface{} `json:"Schedule"`
	Retention   map[string]interface{} `json:"Retention"`
	Options     map[string]interface{} `json:"Options"`
	Filters     []map[string]interface{} `json:"Filters"`
}

type FieldMapping struct {
	DuplicatiField string      `json:"duplicati_field"`
	HBXField       string      `json:"hbx_field"`
	DuplicatiValue interface{} `json:"duplicati_value"`
	HBXValue       interface{} `json:"hbx_value"`
	Supported      bool        `json:"supported"`
}

type UnsupportedItem struct {
	Field     string      `json:"field"`
	Value     interface{} `json:"value"`
	Reason    string      `json:"reason"`
	Action    string      `json:"action"`
}

type ImportResult struct {
	ImportID        uuid.UUID
	SourceHash      string
	Status          ImportStatus
	ResultingJobID  *uuid.UUID
	FieldMappings   []FieldMapping
	UnsupportedItems []UnsupportedItem
	ImportedAt      time.Time
	Idempotent      bool
}

type DuplicatiConfigImporter struct {
	mu            sync.RWMutex
	imports       map[string]*ImportResult
	unsupportedPolicy UnsupportedPolicy
}

func NewImporter(policy UnsupportedPolicy) *DuplicatiConfigImporter {
	return &DuplicatiConfigImporter{
		imports:            make(map[string]*ImportResult),
		unsupportedPolicy: policy,
	}
}

func (imp *DuplicatiConfigImporter) ComputeHash(configData []byte) string {
	h := sha256.Sum256(configData)
	return hex.EncodeToString(h[:])
}

func (imp *DuplicatiConfigImporter) ParseConfig(format SourceFormat, data []byte) (*DuplicatiConfig, error) {
	switch format {
	case FormatJSON:
		return imp.parseJSON(data)
	case FormatSQLite:
		return nil, fmt.Errorf("sqlite format parsing not yet implemented, use JSON export from Duplicati")
	case FormatXML:
		return nil, fmt.Errorf("xml format parsing not yet implemented, use JSON export from Duplicati")
	default:
		return nil, fmt.Errorf("unknown config format: %s", format)
	}
}

func (imp *DuplicatiConfigImporter) parseJSON(data []byte) (*DuplicatiConfig, error) {
	var cfg DuplicatiConfig
	if err := json.Unmarshal(data, &cfg); err != nil {
		return nil, fmt.Errorf("failed to parse JSON config: %w", err)
	}
	if cfg.Name == "" {
		return nil, fmt.Errorf("config missing required field: Name")
	}
	if len(cfg.Sources) == 0 {
		return nil, fmt.Errorf("config missing required field: Sources")
	}
	return &cfg, nil
}

func (imp *DuplicatiConfigImporter) MapFields(cfg *DuplicatiConfig) ([]FieldMapping, []UnsupportedItem) {
	var mappings []FieldMapping
	var unsupported []UnsupportedItem

	mappings = append(mappings, FieldMapping{
		DuplicatiField: "Name", HBXField: "job.name",
		DuplicatiValue: cfg.Name, HBXValue: cfg.Name, Supported: true,
	})

	mappings = append(mappings, FieldMapping{
		DuplicatiField: "Sources", HBXField: "source_config.paths",
		DuplicatiValue: cfg.Sources, HBXValue: cfg.Sources, Supported: true,
	})

	destType, _ := cfg.Destination["Type"].(string)
	mappings = append(mappings, FieldMapping{
		DuplicatiField: "Destination.Type", HBXField: "storage_backend",
		DuplicatiValue: destType, HBXValue: mapDestType(destType), Supported: true,
	})

	if destPath, ok := cfg.Destination["Path"]; ok {
		mappings = append(mappings, FieldMapping{
			DuplicatiField: "Destination.Path", HBXField: "backend_config.root_path",
			DuplicatiValue: destPath, HBXValue: destPath, Supported: true,
		})
	}

	encType, _ := cfg.Encryption["Type"].(string)
	mappings = append(mappings, FieldMapping{
		DuplicatiField: "Encryption.Type", HBXField: "encryption_config.algorithm",
		DuplicatiValue: encType, HBXValue: mapEncryptionType(encType), Supported: true,
	})

	mappings = append(mappings, FieldMapping{
		DuplicatiField: "Compression", HBXField: "compression_config.algorithm",
		DuplicatiValue: cfg.Compression, HBXValue: mapCompression(cfg.Compression), Supported: true,
	})

	if cfg.Schedule != nil {
		mappings = append(mappings, FieldMapping{
			DuplicatiField: "Schedule", HBXField: "schedule_config",
			DuplicatiValue: cfg.Schedule, HBXValue: cfg.Schedule, Supported: true,
		})
	}

	if cfg.Retention != nil {
		mappings = append(mappings, FieldMapping{
			DuplicatiField: "Retention", HBXField: "retention_config",
			DuplicatiValue: cfg.Retention, HBXValue: cfg.Retention, Supported: true,
		})
	}

	if cfg.Options != nil {
		for key, val := range cfg.Options {
			if isOptionSupported(key) {
				mappings = append(mappings, FieldMapping{
					DuplicatiField: "Options." + key, HBXField: "options." + key,
					DuplicatiValue: val, HBXValue: val, Supported: true,
				})
			} else {
				unsupported = append(unsupported, UnsupportedItem{
					Field:  "Options." + key,
					Value:  val,
					Reason: fmt.Sprintf("option '%s' is not supported by HBX", key),
					Action: string(imp.unsupportedPolicy),
				})
			}
		}
	}

	if len(cfg.Filters) > 0 {
		for _, f := range cfg.Filters {
			filterType, _ := f["Type"].(string)
			if !isFilterSupported(filterType) {
				unsupported = append(unsupported, UnsupportedItem{
					Field:  "Filters",
					Value:  f,
					Reason: fmt.Sprintf("filter type '%s' is not supported", filterType),
					Action: string(imp.unsupportedPolicy),
				})
			}
		}
		mappings = append(mappings, FieldMapping{
			DuplicatiField: "Filters", HBXField: "source_config.filters",
			DuplicatiValue: cfg.Filters, HBXValue: cfg.Filters, Supported: true,
		})
	}

	return mappings, unsupported
}

func (imp *DuplicatiConfigImporter) Import(format SourceFormat, configData []byte) (*ImportResult, error) {
	hash := imp.ComputeHash(configData)

	imp.mu.RLock()
	if existing, ok := imp.imports[hash]; ok {
		imp.mu.RUnlock()
		result := *existing
		result.Idempotent = true
		return &result, nil
	}
	imp.mu.RUnlock()

	cfg, err := imp.ParseConfig(format, configData)
	if err != nil {
		result := &ImportResult{
			ImportID:   uuid.New(),
			SourceHash: hash,
			Status:     ImportFailed,
			ImportedAt: time.Now().UTC(),
		}
		imp.mu.Lock()
		imp.imports[hash] = result
		imp.mu.Unlock()
		return result, err
	}

	mappings, unsupported := imp.MapFields(cfg)

	status := ImportSuccess
	if len(unsupported) > 0 {
		if imp.unsupportedPolicy == UnsupportedAbort {
			status = ImportFailed
		} else {
			status = ImportPartial
		}
	}

	var jobID *uuid.UUID
	if status != ImportFailed {
		id := uuid.New()
		jobID = &id
	}

	result := &ImportResult{
		ImportID:         uuid.New(),
		SourceHash:       hash,
		Status:           status,
		ResultingJobID:   jobID,
		FieldMappings:    mappings,
		UnsupportedItems: unsupported,
		ImportedAt:       time.Now().UTC(),
		Idempotent:       false,
	}

	imp.mu.Lock()
	imp.imports[hash] = result
	imp.mu.Unlock()

	return result, nil
}

func (imp *DuplicatiConfigImporter) GetImportByHash(hash string) (*ImportResult, bool) {
	imp.mu.RLock()
	defer imp.mu.RUnlock()
	r, ok := imp.imports[hash]
	return r, ok
}

func (imp *DuplicatiConfigImporter) ListImports() []*ImportResult {
	imp.mu.RLock()
	defer imp.mu.RUnlock()
	result := make([]*ImportResult, 0, len(imp.imports))
	for _, r := range imp.imports {
		result = append(result, r)
	}
	return result
}

func mapDestType(duplicatiType string) string {
	switch duplicatiType {
	case "file", "local":
		return "local"
	case "s3":
		return "s3"
	case "ftp":
		return "ftp"
	case "ftps":
		return "ftps"
	case "webdav":
		return "webdav"
	case "smb":
		return "smb"
	case "azure":
		return "azure_blob"
	case "gcs":
		return "gcs"
	case "openstack":
		return "openstack"
	default:
		return duplicatiType
	}
}

func mapEncryptionType(duplicatiType string) string {
	switch duplicatiType {
	case "AES256", "AES-256":
		return "aes-256-gcm"
	case "PGP":
		return "pgp"
	case "", "none":
		return "none"
	default:
		return duplicatiType
	}
}

func mapCompression(duplicatiComp string) string {
	switch duplicatiComp {
	case "zip":
		return "zstd"
	case "gzip":
		return "gzip"
	case "none":
		return "none"
	default:
		return duplicatiComp
	}
}

var supportedOptions = map[string]bool{
	"blocksize":                 true,
	"threshold":                 true,
	"dblock-size":              true,
	"disable-filetime-check":   true,
	"skip-files-larger-than":   true,
	"exclude-files-attributes": true,
	"upload-unchanged-backups": true,
	"list-verify-uploads":      true,
	"asynchronous-upload-limit": true,
	"asynchronous-concurrent-upload-limit": true,
}

func isOptionSupported(option string) bool {
	return supportedOptions[option]
}

var supportedFilters = map[string]bool{
	"include":   true,
	"exclude":   true,
	"regex":     true,
	"wildcard":  true,
}

func isFilterSupported(filterType string) bool {
	return supportedFilters[filterType]
}