package providers

import (
	"Montscan/config"
	"crypto/tls"
	"fmt"
	"log"
	"net/http"
	"os"
	"path"

	"github.com/studio-b12/gowebdav"
)

func UploadToWebDAV(cfg *config.Config, localPath, remoteFilename string) error {
	if cfg.WebDAVURL == "" || cfg.WebDAVUsername == "" || cfg.WebDAVPassword == "" {
		return fmt.Errorf("WebDAV configuration is incomplete")
	}

	client := gowebdav.NewClient(cfg.WebDAVURL, cfg.WebDAVUsername, cfg.WebDAVPassword)
	if cfg.WebDAVInsecure {
		log.Printf("Warning: InsecureSkipVerify is enabled for WebDAV client. This is not recommended for production environments.")
		transport := &http.Transport{
			TLSClientConfig: &tls.Config{InsecureSkipVerify: true},
		}

		client.SetTransport(transport)
	}

	remotePath := cfg.WebDAVPath
	if err := client.MkdirAll(remotePath, 0755); err != nil {
		log.Printf("Warning: could not create remote directory (may already exist): %v", err)
	}

	data, err := os.ReadFile(localPath)
	if err != nil {
		return fmt.Errorf("failed to read local file: %w", err)
	}

	fullRemotePath := path.Join(remotePath, remoteFilename)
	log.Printf("Uploading to WebDAV: %s", cfg.WebDAVURL+fullRemotePath)

	if err := client.Write(fullRemotePath, data, 0644); err != nil {
		return fmt.Errorf("failed to upload to WebDAV: %w", err)
	}

	log.Printf("Successfully uploaded to WebDAV: %s", fullRemotePath)
	return nil
}
