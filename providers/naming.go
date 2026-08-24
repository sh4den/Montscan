package providers

import (
	"fmt"
	"path/filepath"
	"strings"
)

const MaxNameAttempts = 10000

func CandidateName(filename string, attempt int) string {
	if attempt == 0 {
		return filename
	}
	ext := filepath.Ext(filename)
	stem := strings.TrimSuffix(filename, ext)
	return fmt.Sprintf("%s_%d%s", stem, attempt, ext)
}
