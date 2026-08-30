//go:build !windows

package testorch

func hideFile(path string) error {
	return nil
}