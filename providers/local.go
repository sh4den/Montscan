package providers

import (
	"fmt"
	"io"
	"os"
	"path/filepath"

	"Montscan/config"
)

func MoveLocal(cfg *config.Config, localPath, newFilename string) error {
	var destDir string
	if cfg.FolderOutputDir != "" {
		destDir = cfg.FolderOutputDir
	} else {
		destDir = filepath.Dir(localPath)
	}

	dest := filepath.Join(destDir, newFilename)
	if err := os.Rename(localPath, dest); err == nil {
		return nil
	}

	// Fallback for cross-device moves (different mount points).
	return copyAndDelete(localPath, dest)
}

func copyAndDelete(src, dst string) error {
	in, err := os.Open(src)
	if err != nil {
		return fmt.Errorf("local move: open source: %w", err)
	}
	defer func(in *os.File) {
		err := in.Close()
		if err != nil {
			fmt.Printf("local move: close source: %v\n", err)
		}
	}(in)

	out, err := os.Create(dst)
	if err != nil {
		return fmt.Errorf("local move: create dest: %w", err)
	}
	defer func(out *os.File) {
		err := out.Close()
		if err != nil {
			fmt.Printf("local move: close dest: %v\n", err)
		}
	}(out)

	if _, err := io.Copy(out, in); err != nil {
		_ = os.Remove(dst)
		return fmt.Errorf("local move: copy: %w", err)
	}

	if err := out.Close(); err != nil {
		_ = os.Remove(dst)
		return fmt.Errorf("local move: close dest: %w", err)
	}

	return os.Remove(src)
}
