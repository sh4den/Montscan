package agent

import (
	"fmt"
	"log"
	"os"
	"path"

	"github.com/studio-b12/gowebdav"
)

func (a *Agent) UploadToWebDAV(localPath, remoteFilename string) error {
	if a.config.WebDAVURL == "" || a.config.WebDAVUsername == "" || a.config.WebDAVPassword == "" {
		return fmt.Errorf("WebDAV credentials not configured")
	}

	// Use the WebDAV URL directly as provided by the user
	webdavURL := a.config.WebDAVURL

	client := gowebdav.NewClient(webdavURL, a.config.WebDAVUsername, a.config.WebDAVPassword)

	remotePath := a.config.WebDAVPath
	if err := client.MkdirAll(remotePath, 0755); err != nil {
		log.Printf("Warning: could not create remote directory (may already exist): %v", err)
	}

	data, err := os.ReadFile(localPath)
	if err != nil {
		return fmt.Errorf("failed to read local file: %w", err)
	}

	fullRemotePath := path.Join(remotePath, remoteFilename)
	log.Printf("Uploading to WebDAV: %s", fullRemotePath)

	if err := client.Write(fullRemotePath, data, 0644); err != nil {
		return fmt.Errorf("failed to upload to WebDAV: %w", err)
	}

	log.Printf("Successfully uploaded to WebDAV: %s", fullRemotePath)
	return nil
}
