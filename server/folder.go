package server

import (
	"log"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"Montscan/agent"
	"Montscan/config"
)

func StartFolderServer(cfg *config.Config, ag *agent.Agent) error {
	if err := os.MkdirAll(cfg.FolderInputDir, 0o755); err != nil {
		return err
	}

	if cfg.FolderOutputDir != "" {
		if err := os.MkdirAll(cfg.FolderOutputDir, 0o755); err != nil {
			return err
		}
	}

	mode := "rename in place"
	if cfg.FolderOutputDir != "" {
		mode = "move to " + cfg.FolderOutputDir
	}
	log.Printf("Starting folder watcher: input=%s mode=%s interval=%ds", cfg.FolderInputDir, mode, cfg.FolderPollIntervalSec)

	inProgress := &sync.Map{}
	interval := time.Duration(cfg.FolderPollIntervalSec) * time.Second

	for {
		pollLocalFolder(ag, cfg.FolderInputDir, inProgress)
		time.Sleep(interval)
	}
}

func pollLocalFolder(ag *agent.Agent, inputDir string, inProgress *sync.Map) {
	entries, err := os.ReadDir(inputDir)
	if err != nil {
		log.Printf("Folder watcher: error reading %s: %v", inputDir, err)
		return
	}

	for _, entry := range entries {
		if entry.IsDir() || !strings.EqualFold(filepath.Ext(entry.Name()), ".pdf") {
			continue
		}

		filePath := filepath.Join(inputDir, entry.Name())
		if _, loaded := inProgress.LoadOrStore(filePath, true); loaded {
			continue
		}

		go func(path string) {
			defer inProgress.Delete(path)
			ag.ProcessDocument(path)
		}(filePath)
	}
}
