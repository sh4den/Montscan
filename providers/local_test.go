package providers

import (
	"os"
	"path/filepath"
	"testing"

	"Montscan/config"
)

func writeFile(t *testing.T, path, content string) {
	t.Helper()
	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		t.Fatal(err)
	}
}

func readFile(t *testing.T, path string) string {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	return string(data)
}

func TestCandidateName(t *testing.T) {
	if got := CandidateName("DOC.pdf", 0); got != "DOC.pdf" {
		t.Errorf("attempt 0: got %q", got)
	}
	if got := CandidateName("DOC.pdf", 2); got != "DOC_2.pdf" {
		t.Errorf("attempt 2: got %q", got)
	}
}

func TestMoveLocalNoCollision(t *testing.T) {
	in, out := t.TempDir(), t.TempDir()
	src := filepath.Join(in, "scan.pdf")
	writeFile(t, src, "new")

	cfg := &config.Config{FolderOutputDir: out}
	if err := MoveLocal(cfg, src, "DOC.pdf"); err != nil {
		t.Fatal(err)
	}

	if got := readFile(t, filepath.Join(out, "DOC.pdf")); got != "new" {
		t.Errorf("moved content: got %q", got)
	}
	if _, err := os.Stat(src); !os.IsNotExist(err) {
		t.Errorf("source still exists: %v", err)
	}
}

func TestMoveLocalNeverOverwrites(t *testing.T) {
	in, out := t.TempDir(), t.TempDir()
	writeFile(t, filepath.Join(out, "DOC.pdf"), "original")
	writeFile(t, filepath.Join(out, "DOC_1.pdf"), "original 1")
	src := filepath.Join(in, "scan.pdf")
	writeFile(t, src, "new")

	cfg := &config.Config{FolderOutputDir: out}
	if err := MoveLocal(cfg, src, "DOC.pdf"); err != nil {
		t.Fatal(err)
	}

	if got := readFile(t, filepath.Join(out, "DOC.pdf")); got != "original" {
		t.Errorf("existing file was overwritten: got %q", got)
	}
	if got := readFile(t, filepath.Join(out, "DOC_1.pdf")); got != "original 1" {
		t.Errorf("existing file was overwritten: got %q", got)
	}
	if got := readFile(t, filepath.Join(out, "DOC_2.pdf")); got != "new" {
		t.Errorf("suffixed file: got %q", got)
	}
}

func TestMoveLocalRenameInPlace(t *testing.T) {
	in := t.TempDir()
	writeFile(t, filepath.Join(in, "INVOICE.pdf"), "other")
	src := filepath.Join(in, "scan.pdf")
	writeFile(t, src, "new")

	cfg := &config.Config{}
	if err := MoveLocal(cfg, src, "INVOICE.pdf"); err != nil {
		t.Fatal(err)
	}

	if got := readFile(t, filepath.Join(in, "INVOICE.pdf")); got != "other" {
		t.Errorf("existing file was overwritten: got %q", got)
	}
	if got := readFile(t, filepath.Join(in, "INVOICE_1.pdf")); got != "new" {
		t.Errorf("suffixed file: got %q", got)
	}
}

func TestMoveLocalSameNameKeepsFile(t *testing.T) {
	in := t.TempDir()
	src := filepath.Join(in, "INVOICE.pdf")
	writeFile(t, src, "content")

	cfg := &config.Config{}
	if err := MoveLocal(cfg, src, "INVOICE.pdf"); err != nil {
		t.Fatal(err)
	}

	if got := readFile(t, src); got != "content" {
		t.Errorf("file was lost or changed: got %q", got)
	}
	if _, err := os.Stat(filepath.Join(in, "INVOICE_1.pdf")); !os.IsNotExist(err) {
		t.Errorf("unexpected suffixed copy created")
	}
}

func TestMoveLocalCrossDeviceFallback(t *testing.T) {
	in, out := t.TempDir(), t.TempDir()
	writeFile(t, filepath.Join(out, "DOC.pdf"), "original")
	src := filepath.Join(in, "scan.pdf")
	writeFile(t, src, "new")

	if err := copyAndClaim(t, src, out, "DOC.pdf"); err != nil {
		t.Fatal(err)
	}
}

func copyAndClaim(t *testing.T, src, destDir, name string) error {
	t.Helper()
	for attempt := 0; attempt < MaxNameAttempts; attempt++ {
		dest := filepath.Join(destDir, CandidateName(name, attempt))
		placeholder, err := os.OpenFile(dest, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0644)
		if os.IsExist(err) {
			continue
		}
		if err != nil {
			return err
		}
		if err := copyInto(placeholder, src); err != nil {
			_ = placeholder.Close()
			return err
		}
		if err := placeholder.Close(); err != nil {
			return err
		}
		if got := readFile(t, dest); got != "new" {
			t.Errorf("copied content: got %q", got)
		}
		if got := readFile(t, filepath.Join(destDir, name)); got != "original" {
			t.Errorf("existing file was overwritten: got %q", got)
		}
		return nil
	}
	return os.ErrExist
}
