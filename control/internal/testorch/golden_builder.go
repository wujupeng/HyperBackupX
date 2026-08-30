package testorch

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"strings"
)

type GoldenFixtureType string

const (
	FixtureOneByte     GoldenFixtureType = "one_byte"
	FixtureEmpty       GoldenFixtureType = "empty"
	FixtureLarge       GoldenFixtureType = "large"
	FixtureChineseName GoldenFixtureType = "chinese_filename"
	FixtureLongPath    GoldenFixtureType = "long_path"
	FixtureHidden      GoldenFixtureType = "hidden"
	FixtureDuplicate   GoldenFixtureType = "duplicate"
	FixtureModified    GoldenFixtureType = "modified"
	FixtureDeleted     GoldenFixtureType = "deleted"
	FixtureLocked      GoldenFixtureType = "locked"
)

type GoldenFixture struct {
	Type         GoldenFixtureType `json:"type"`
	RelativePath string            `json:"relative_path"`
	Size         int64             `json:"size"`
	Content      []byte            `json:"content,omitempty"`
	IsDeleted    bool              `json:"is_deleted"`
	IsLocked     bool              `json:"is_locked"`
	IsHidden     bool              `json:"is_hidden"`
	SHA256       string            `json:"sha256"`
}

type GoldenDataset struct {
	Name     string          `json:"name"`
	Fixtures []GoldenFixture `json:"fixtures"`
}

type GoldenDatasetBuilder struct {
	rootDir string
}

func NewGoldenDatasetBuilder(rootDir string) *GoldenDatasetBuilder {
	return &GoldenDatasetBuilder{rootDir: rootDir}
}

func (b *GoldenDatasetBuilder) Build() *GoldenDataset {
	dataset := &GoldenDataset{
		Name:     "hbx-golden-dataset",
		Fixtures: make([]GoldenFixture, 0),
	}

	dataset.Fixtures = append(dataset.Fixtures, b.buildOneByteFiles()...)
	dataset.Fixtures = append(dataset.Fixtures, b.buildEmptyFiles()...)
	dataset.Fixtures = append(dataset.Fixtures, b.buildLargeFiles()...)
	dataset.Fixtures = append(dataset.Fixtures, b.buildChineseFilenameFiles()...)
	dataset.Fixtures = append(dataset.Fixtures, b.buildLongPathFiles()...)
	dataset.Fixtures = append(dataset.Fixtures, b.buildHiddenFiles()...)
	dataset.Fixtures = append(dataset.Fixtures, b.buildDuplicateFiles()...)
	dataset.Fixtures = append(dataset.Fixtures, b.buildModifiedFiles()...)
	dataset.Fixtures = append(dataset.Fixtures, b.buildDeletedFiles()...)
	dataset.Fixtures = append(dataset.Fixtures, b.buildLockedFiles()...)

	return dataset
}

func (b *GoldenDatasetBuilder) buildOneByteFiles() []GoldenFixture {
	contents := [][]byte{{'A'}, {0x00}, {0xFF}}
	fixtures := make([]GoldenFixture, 0, len(contents))
	for i, c := range contents {
		path := fmt.Sprintf("one_byte/file_%d.bin", i)
		fixtures = append(fixtures, GoldenFixture{
			Type:         FixtureOneByte,
			RelativePath: path,
			Size:         1,
			Content:      c,
			SHA256:       sha256hex(c),
		})
	}
	return fixtures
}

func (b *GoldenDatasetBuilder) buildEmptyFiles() []GoldenFixture {
	paths := []string{"empty/empty.txt", "empty/zero.dat", "empty/.gitkeep"}
	fixtures := make([]GoldenFixture, 0, len(paths))
	for _, p := range paths {
		fixtures = append(fixtures, GoldenFixture{
			Type:         FixtureEmpty,
			RelativePath: p,
			Size:         0,
			Content:      []byte{},
			SHA256:       sha256hex([]byte{}),
		})
	}
	return fixtures
}

func (b *GoldenDatasetBuilder) buildLargeFiles() []GoldenFixture {
	sizes := []struct {
		name string
		size int64
	}{
		{"large/1mb.bin", 1024 * 1024},
		{"large/10mb.bin", 10 * 1024 * 1024},
		{"large/100mb.bin", 100 * 1024 * 1024},
	}
	fixtures := make([]GoldenFixture, 0, len(sizes))
	for _, s := range sizes {
		fixtures = append(fixtures, GoldenFixture{
			Type:         FixtureLarge,
			RelativePath: s.name,
			Size:         s.size,
			SHA256:       "",
		})
	}
	return fixtures
}

func (b *GoldenDatasetBuilder) buildChineseFilenameFiles() []GoldenFixture {
	names := []string{
		"chinese/中文文档.txt",
		"chinese/数据备份/报表.csv",
		"chinese/测试文件_2024.log",
		"chinese/备份目录/系统配置.json",
	}
	fixtures := make([]GoldenFixture, 0, len(names))
	for _, n := range names {
		content := []byte("chinese content for " + n)
		fixtures = append(fixtures, GoldenFixture{
			Type:         FixtureChineseName,
			RelativePath: n,
			Size:         int64(len(content)),
			Content:      content,
			SHA256:       sha256hex(content),
		})
	}
	return fixtures
}

func (b *GoldenDatasetBuilder) buildLongPathFiles() []GoldenFixture {
	fixtures := make([]GoldenFixture, 0, 3)

	deep := strings.Repeat("subdir/", 40)
	fixtures = append(fixtures, GoldenFixture{
		Type:         FixtureLongPath,
		RelativePath: fmt.Sprintf("longpath/%sdeep_file.txt", deep),
		Size:         10,
		Content:      []byte("longpath1"),
		SHA256:       sha256hex([]byte("longpath1")),
	})

	longName := strings.Repeat("a", 260) + ".txt"
	fixtures = append(fixtures, GoldenFixture{
		Type:         FixtureLongPath,
		RelativePath: "longpath/" + longName,
		Size:         10,
		Content:      []byte("longpath2"),
		SHA256:       sha256hex([]byte("longpath2")),
	})

	deep2 := strings.Repeat("d/", 130)
	fixtures = append(fixtures, GoldenFixture{
		Type:         FixtureLongPath,
		RelativePath: fmt.Sprintf("longpath/%snested.bin", deep2),
		Size:         10,
		Content:      []byte("longpath3"),
		SHA256:       sha256hex([]byte("longpath3")),
	})

	return fixtures
}

func (b *GoldenDatasetBuilder) buildHiddenFiles() []GoldenFixture {
	paths := []string{
		"hidden/.hidden_file",
		"hidden/.config",
		"hidden/.secret",
	}
	fixtures := make([]GoldenFixture, 0, len(paths))
	for _, p := range paths {
		content := []byte("hidden content")
		fixtures = append(fixtures, GoldenFixture{
			Type:         FixtureHidden,
			RelativePath: p,
			Size:         int64(len(content)),
			Content:      content,
			IsHidden:     true,
			SHA256:       sha256hex(content),
		})
	}
	return fixtures
}

func (b *GoldenDatasetBuilder) buildDuplicateFiles() []GoldenFixture {
	content := []byte("duplicate content - same bytes")
	fixtures := []GoldenFixture{
		{
			Type:         FixtureDuplicate,
			RelativePath: "duplicate/original.txt",
			Size:         int64(len(content)),
			Content:      content,
			SHA256:       sha256hex(content),
		},
		{
			Type:         FixtureDuplicate,
			RelativePath: "duplicate/copy1.txt",
			Size:         int64(len(content)),
			Content:      content,
			SHA256:       sha256hex(content),
		},
		{
			Type:         FixtureDuplicate,
			RelativePath: "duplicate/copy2.txt",
			Size:         int64(len(content)),
			Content:      content,
			SHA256:       sha256hex(content),
		},
	}
	return fixtures
}

func (b *GoldenDatasetBuilder) buildModifiedFiles() []GoldenFixture {
	fixtures := []GoldenFixture{
		{
			Type:         FixtureModified,
			RelativePath: "modified/v1.txt",
			Size:         3,
			Content:      []byte("v1\n"),
			SHA256:       sha256hex([]byte("v1\n")),
		},
		{
			Type:         FixtureModified,
			RelativePath: "modified/v2.txt",
			Size:         19,
			Content:      []byte("v2-modified-content"),
			SHA256:       sha256hex([]byte("v2-modified-content")),
		},
	}
	return fixtures
}

func (b *GoldenDatasetBuilder) buildDeletedFiles() []GoldenFixture {
	fixtures := []GoldenFixture{
		{
			Type:         FixtureDeleted,
			RelativePath: "deleted/will_be_deleted.txt",
			Size:         7,
			Content:      []byte("deleted"),
			IsDeleted:    true,
			SHA256:       sha256hex([]byte("deleted")),
		},
		{
			Type:         FixtureDeleted,
			RelativePath: "deleted/also_removed.dat",
			Size:         6,
			Content:      []byte("removed"),
			IsDeleted:    true,
			SHA256:       sha256hex([]byte("removed")),
		},
	}
	return fixtures
}

func (b *GoldenDatasetBuilder) buildLockedFiles() []GoldenFixture {
	fixtures := []GoldenFixture{
		{
			Type:         FixtureLocked,
			RelativePath: "locked/locked_file1.txt",
			Size:         7,
			Content:      []byte("locked1"),
			IsLocked:     true,
			SHA256:       sha256hex([]byte("locked1")),
		},
		{
			Type:         FixtureLocked,
			RelativePath: "locked/locked_file2.txt",
			Size:         7,
			Content:      []byte("locked2"),
			IsLocked:     true,
			SHA256:       sha256hex([]byte("locked2")),
		},
	}
	return fixtures
}

func (d *GoldenDataset) Count() int {
	return len(d.Fixtures)
}

func (d *GoldenDataset) GetByType(t GoldenFixtureType) []GoldenFixture {
	var result []GoldenFixture
	for _, f := range d.Fixtures {
		if f.Type == t {
			result = append(result, f)
		}
	}
	return result
}

func (d *GoldenDataset) GetTypes() []GoldenFixtureType {
	seen := make(map[GoldenFixtureType]bool)
	var types []GoldenFixtureType
	for _, f := range d.Fixtures {
		if !seen[f.Type] {
			seen[f.Type] = true
			types = append(types, f.Type)
		}
	}
	return types
}

func (d *GoldenDataset) TotalSize() int64 {
	var total int64
	for _, f := range d.Fixtures {
		if !f.IsDeleted {
			total += f.Size
		}
	}
	return total
}

func sha256hex(data []byte) string {
	h := sha256.Sum256(data)
	return hex.EncodeToString(h[:])
}

func (b *GoldenDatasetBuilder) Materialize(dataset *GoldenDataset, destDir string) error {
	if err := osMkdirAll(destDir); err != nil {
		return fmt.Errorf("create dest dir: %w", err)
	}

	for _, fixture := range dataset.Fixtures {
		if fixture.IsDeleted {
			continue
		}

		fullPath := joinPath(destDir, fixture.RelativePath)

		if len(filepathBase(fullPath)) > 255 {
			continue
		}

		parentDir := dirOf(fullPath)
		if err := osMkdirAll(parentDir); err != nil {
			return fmt.Errorf("create parent dir for %s: %w", fixture.RelativePath, err)
		}

		if fixture.IsLocked {
			if err := osWriteFileLocked(fullPath, fixture.Content); err != nil {
				return fmt.Errorf("write locked file %s: %w", fixture.RelativePath, err)
			}
		} else {
			if err := osWriteFile(fullPath, fixture.Content); err != nil {
				return fmt.Errorf("write file %s: %w", fixture.RelativePath, err)
			}
		}

		if fixture.IsHidden {
			osSetHidden(fullPath)
		}
	}
	return nil
}
