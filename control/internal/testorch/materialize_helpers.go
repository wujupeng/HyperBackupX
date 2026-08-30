package testorch

import (
	"os"
	"path/filepath"
	"strings"
)

func osMkdirAll(dir string) error {
	return os.MkdirAll(dir, 0755)
}

func osWriteFile(path string, content []byte) error {
	return os.WriteFile(path, content, 0644)
}

func osWriteFileLocked(path string, content []byte) error {
	return os.WriteFile(path, content, 0644)
}

func osSetHidden(path string) {
	_ = hideFile(path)
}

func joinPath(base, rel string) string {
	if strings.HasPrefix(rel, "/") {
		rel = rel[1:]
	}
	return filepath.Join(base, rel)
}

func dirOf(path string) string {
	return filepath.Dir(path)
}

func osStat(path string) (os.FileInfo, error) {
	return os.Stat(path)
}

func filepathBase(path string) string {
	return filepath.Base(path)
}