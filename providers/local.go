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

	for attempt := 0; attempt < MaxNameAttempts; attempt++ {
		dest := filepath.Join(destDir, CandidateName(newFilename, attempt))

		if dest == filepath.Clean(localPath) {
			return nil
		}

		placeholder, err := os.OpenFile(dest, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0644)
		if err != nil {
			if os.IsExist(err) {
				continue
			}
			return fmt.Errorf("local move: claim dest: %w", err)
		}

		if err := os.Rename(localPath, dest); err == nil {
			_ = placeholder.Close()
			return nil
		}

		// Fallback for cross-device moves (different mount points).
		if err := copyInto(placeholder, localPath); err != nil {
			_ = placeholder.Close()
			_ = os.Remove(dest)
			return err
		}
		if err := placeholder.Close(); err != nil {
			_ = os.Remove(dest)
			return fmt.Errorf("local move: close dest: %w", err)
		}
		return os.Remove(localPath)
	}

	return fmt.Errorf("local move: no free filename found for %s in %s", newFilename, destDir)
}

func copyInto(out *os.File, src string) error {
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

	if _, err := io.Copy(out, in); err != nil {
		return fmt.Errorf("local move: copy: %w", err)
	}

	return nil
}
