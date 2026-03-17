package server

import (
	"fmt"
	"io"
	"log"
	"net"
	"os"
	"path"
	"path/filepath"
	"strconv"
	"strings"
	"time"

	"Montscan/agent"
	"Montscan/config"

	"github.com/hirochachacha/go-smb2"
)

func StartSambaServer(cfg *config.Config, ag *agent.Agent) error {
	if !cfg.SambaServerEnabled {
		return nil
	}

	if cfg.SambaServerHost == "" || cfg.SambaServerUsername == "" || cfg.SambaServerPassword == "" || cfg.SambaServerShare == "" || cfg.SambaServerPath == "" {
		return fmt.Errorf("samba server configuration is incomplete")
	}

	workDir := filepath.Join(cfg.SambaServerWorkDir, "samba-server")
	if err := os.MkdirAll(workDir, 0o755); err != nil {
		return fmt.Errorf("failed to create samba server work dir: %w", err)
	}

	interval := time.Duration(cfg.SambaServerPollIntervalSec) * time.Second
	log.Printf("Starting Samba server poller on %s:%d share=%s path=%s interval=%s", cfg.SambaServerHost, cfg.SambaServerPort, cfg.SambaServerShare, cfg.SambaServerPath, interval)

	for {
		if err := pollSambaServer(cfg, ag, workDir); err != nil {
			log.Printf("Samba server poll failed: %v", err)
		}
		time.Sleep(interval)
	}
}

func pollSambaServer(cfg *config.Config, ag *agent.Agent, workDir string) error {
	addr := net.JoinHostPort(cfg.SambaServerHost, strconv.Itoa(cfg.SambaServerPort))
	con, err := net.Dial("tcp", addr)
	if err != nil {
		return fmt.Errorf("failed to connect to SMB server: %w", err)
	}
	defer func() {
		_ = con.Close()
	}()

	dialer := &smb2.Dialer{
		Initiator: &smb2.NTLMInitiator{
			User:     cfg.SambaServerUsername,
			Password: cfg.SambaServerPassword,
		},
	}

	smbConn, err := dialer.Dial(con)
	if err != nil {
		return fmt.Errorf("failed to negotiate SMB session: %w", err)
	}
	defer func() {
		_ = smbConn.Logoff()
	}()

	share, err := smbConn.Mount(cfg.SambaServerShare)
	if err != nil {
		return fmt.Errorf("failed to mount SMB share: %w", err)
	}
	defer func() {
		_ = share.Umount()
	}()

	entries, err := share.ReadDir(cfg.SambaServerPath)
	if err != nil {
		return fmt.Errorf("failed to read smb path %q: %w", cfg.SambaServerPath, err)
	}

	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}

		name := entry.Name()
		if !strings.HasSuffix(strings.ToLower(name), ".pdf") {
			continue
		}

		remotePath := path.Join(cfg.SambaServerPath, name)
		localPath, err := downloadSMBFile(share, remotePath, workDir)
		if err != nil {
			log.Printf("Failed to download SMB file %s: %v", remotePath, err)
			continue
		}

		if cfg.SambaServerDeleteAfterRead {
			if err := share.Remove(remotePath); err != nil {
				log.Printf("Failed to remove SMB source file %s: %v", remotePath, err)
			}
		}

		if ag == nil {
			log.Println("Document processor not initialized")
			continue
		}

		go ag.ProcessDocument(localPath)
	}

	return nil
}

func downloadSMBFile(share *smb2.Share, remotePath, workDir string) (string, error) {
	remoteFile, err := share.Open(remotePath)
	if err != nil {
		return "", err
	}
	defer func() {
		_ = remoteFile.Close()
	}()

	localPath := filepath.Join(workDir, fmt.Sprintf("%d-%s", time.Now().UnixNano(), filepath.Base(remotePath)))
	localFile, err := os.Create(localPath)
	if err != nil {
		return "", err
	}
	defer func() {
		_ = localFile.Close()
	}()

	if _, err := io.Copy(localFile, remoteFile); err != nil {
		return "", err
	}

	return localPath, nil
}
