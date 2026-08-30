package testorch

import (
	"testing"
)

func TestGoldenDatasetBuild(t *testing.T) {
	builder := NewGoldenDatasetBuilder("/tmp/golden")
	dataset := builder.Build()

	if dataset.Count() == 0 {
		t.Error("expected non-empty golden dataset")
	}
}

func TestGoldenDatasetAllTypesPresent(t *testing.T) {
	builder := NewGoldenDatasetBuilder("/tmp/golden")
	dataset := builder.Build()

	requiredTypes := []GoldenFixtureType{
		FixtureOneByte,
		FixtureEmpty,
		FixtureLarge,
		FixtureChineseName,
		FixtureLongPath,
		FixtureHidden,
		FixtureDuplicate,
		FixtureModified,
		FixtureDeleted,
		FixtureLocked,
	}

	for _, rt := range requiredTypes {
		fixtures := dataset.GetByType(rt)
		if len(fixtures) == 0 {
			t.Errorf("expected fixtures of type %s, got none", rt)
		}
	}
}

func TestGoldenDatasetTypeCount(t *testing.T) {
	builder := NewGoldenDatasetBuilder("/tmp/golden")
	dataset := builder.Build()

	types := dataset.GetTypes()
	if len(types) != 10 {
		t.Errorf("expected 10 fixture types, got %d", len(types))
	}
}

func TestGoldenDatasetOneByteFiles(t *testing.T) {
	builder := NewGoldenDatasetBuilder("/tmp/golden")
	dataset := builder.Build()

	oneByte := dataset.GetByType(FixtureOneByte)
	if len(oneByte) != 3 {
		t.Errorf("expected 3 one-byte files, got %d", len(oneByte))
	}
	for _, f := range oneByte {
		if f.Size != 1 {
			t.Errorf("expected size 1, got %d", f.Size)
		}
		if len(f.Content) != 1 {
			t.Errorf("expected content length 1, got %d", len(f.Content))
		}
	}
}

func TestGoldenDatasetEmptyFiles(t *testing.T) {
	builder := NewGoldenDatasetBuilder("/tmp/golden")
	dataset := builder.Build()

	empty := dataset.GetByType(FixtureEmpty)
	if len(empty) != 3 {
		t.Errorf("expected 3 empty files, got %d", len(empty))
	}
	for _, f := range empty {
		if f.Size != 0 {
			t.Errorf("expected size 0, got %d", f.Size)
		}
	}
}

func TestGoldenDatasetLargeFiles(t *testing.T) {
	builder := NewGoldenDatasetBuilder("/tmp/golden")
	dataset := builder.Build()

	large := dataset.GetByType(FixtureLarge)
	if len(large) != 3 {
		t.Errorf("expected 3 large files, got %d", len(large))
	}
	for _, f := range large {
		if f.Size < 1024*1024 {
			t.Errorf("expected size >= 1MB, got %d", f.Size)
		}
	}
}

func TestGoldenDatasetChineseFilenames(t *testing.T) {
	builder := NewGoldenDatasetBuilder("/tmp/golden")
	dataset := builder.Build()

	chinese := dataset.GetByType(FixtureChineseName)
	if len(chinese) != 4 {
		t.Errorf("expected 4 chinese filename files, got %d", len(chinese))
	}
}

func TestGoldenDatasetLongPaths(t *testing.T) {
	builder := NewGoldenDatasetBuilder("/tmp/golden")
	dataset := builder.Build()

	longPath := dataset.GetByType(FixtureLongPath)
	if len(longPath) != 3 {
		t.Errorf("expected 3 long path files, got %d", len(longPath))
	}
	for _, f := range longPath {
		if len(f.RelativePath) <= 260 {
			t.Errorf("expected path length > 260, got %d", len(f.RelativePath))
		}
	}
}

func TestGoldenDatasetHiddenFiles(t *testing.T) {
	builder := NewGoldenDatasetBuilder("/tmp/golden")
	dataset := builder.Build()

	hidden := dataset.GetByType(FixtureHidden)
	if len(hidden) != 3 {
		t.Errorf("expected 3 hidden files, got %d", len(hidden))
	}
	for _, f := range hidden {
		if !f.IsHidden {
			t.Errorf("expected hidden flag true for %s", f.RelativePath)
		}
	}
}

func TestGoldenDatasetDuplicateFiles(t *testing.T) {
	builder := NewGoldenDatasetBuilder("/tmp/golden")
	dataset := builder.Build()

	dup := dataset.GetByType(FixtureDuplicate)
	if len(dup) != 3 {
		t.Errorf("expected 3 duplicate files, got %d", len(dup))
	}
	if len(dup) >= 2 {
		if dup[0].SHA256 != dup[1].SHA256 {
			t.Error("expected duplicate files to have same SHA256")
		}
	}
}

func TestGoldenDatasetDeletedFiles(t *testing.T) {
	builder := NewGoldenDatasetBuilder("/tmp/golden")
	dataset := builder.Build()

	deleted := dataset.GetByType(FixtureDeleted)
	if len(deleted) != 2 {
		t.Errorf("expected 2 deleted files, got %d", len(deleted))
	}
	for _, f := range deleted {
		if !f.IsDeleted {
			t.Errorf("expected deleted flag true for %s", f.RelativePath)
		}
	}
}

func TestGoldenDatasetLockedFiles(t *testing.T) {
	builder := NewGoldenDatasetBuilder("/tmp/golden")
	dataset := builder.Build()

	locked := dataset.GetByType(FixtureLocked)
	if len(locked) != 2 {
		t.Errorf("expected 2 locked files, got %d", len(locked))
	}
	for _, f := range locked {
		if !f.IsLocked {
			t.Errorf("expected locked flag true for %s", f.RelativePath)
		}
	}
}

func TestGoldenDatasetTotalSize(t *testing.T) {
	builder := NewGoldenDatasetBuilder("/tmp/golden")
	dataset := builder.Build()

	total := dataset.TotalSize()
	if total <= 0 {
		t.Errorf("expected positive total size, got %d", total)
	}
}