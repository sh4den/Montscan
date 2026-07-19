package providers

import (
	"Montscan/config"
	"bytes"
	"crypto/tls"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"path"

	"github.com/studio-b12/gowebdav"
)

// webdavDebugBodyLimit caps how much of an error response body gets logged.
const webdavDebugBodyLimit = 4096

// debugTransport logs the response body of failed WebDAV requests.
// gowebdav only surfaces the status code (e.g. "Write foo.pdf: 502") and
// discards the body, which usually holds the actual reason (proxy error
// page, server error message, etc.), so we capture it here instead.
type debugTransport struct {
	base http.RoundTripper
}

func (t *debugTransport) RoundTrip(req *http.Request) (*http.Response, error) {
	resp, err := t.base.RoundTrip(req)
	if err != nil {
		log.Printf("WebDAV debug: %s %s failed: %v", req.Method, req.URL, err)
		return resp, err
	}

	if resp.StatusCode >= 400 {
		body, readErr := io.ReadAll(io.LimitReader(resp.Body, webdavDebugBodyLimit))
		resp.Body.Close()
		if readErr != nil {
			log.Printf("WebDAV debug: %s %s -> %d (failed to read response body: %v)", req.Method, req.URL, resp.StatusCode, readErr)
		} else {
			log.Printf("WebDAV debug: %s %s -> %d, response body: %s", req.Method, req.URL, resp.StatusCode, body)
		}
		resp.Body = io.NopCloser(bytes.NewReader(body))
	}

	return resp, err
}

func newWebDAVClient(cfg *config.Config) *gowebdav.Client {
	client := gowebdav.NewClient(cfg.WebDAVURL, cfg.WebDAVUsername, cfg.WebDAVPassword)

	var transport http.RoundTripper = http.DefaultTransport
	if cfg.WebDAVInsecure {
		transport = &http.Transport{
			TLSClientConfig: &tls.Config{InsecureSkipVerify: true},
		}
	}
	client.SetTransport(&debugTransport{base: transport})
	return client
}

func UploadToWebDAV(cfg *config.Config, localPath, remoteFilename string) error {
	if cfg.WebDAVURL == "" || cfg.WebDAVUsername == "" || cfg.WebDAVPassword == "" {
		return fmt.Errorf("WebDAV configuration is incomplete")
	}

	if cfg.WebDAVInsecure {
		log.Printf("Warning: InsecureSkipVerify is enabled for WebDAV client. This is not recommended for production environments.")
	}

	client := newWebDAVClient(cfg)

	remotePath := cfg.WebDAVPath
	if err := client.MkdirAll(remotePath, 0755); err != nil {
		log.Printf("Warning: could not create remote directory (may already exist): %v", err)
	}

	data, err := os.ReadFile(localPath)
	if err != nil {
		return fmt.Errorf("failed to read local file: %w", err)
	}

	writeClient := newWebDAVClient(cfg)
	writeClient.SetHeader("If-None-Match", "*")

	for attempt := 0; attempt < MaxNameAttempts; attempt++ {
		fullRemotePath := path.Join(remotePath, CandidateName(remoteFilename, attempt))

		if _, err := client.Stat(fullRemotePath); err == nil {
			continue
		} else if !gowebdav.IsErrNotFound(err) {
			return fmt.Errorf("failed to stat remote file %s: %w", fullRemotePath, err)
		}

		log.Printf("Uploading to WebDAV: %s", cfg.WebDAVURL+fullRemotePath)

		err := writeClient.Write(fullRemotePath, data, 0644)
		if err == nil {
			log.Printf("Successfully uploaded to WebDAV: %s", fullRemotePath)
			return nil
		}
		if gowebdav.IsErrCode(err, 412) {
			continue
		}
		return fmt.Errorf("failed to upload to WebDAV: %w", err)
	}

	return fmt.Errorf("no free filename found on WebDAV for %s", remoteFilename)
}
