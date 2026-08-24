package server

import (
	"fmt"
	"io"
	"log"
	"os"
	"path/filepath"
	"strings"

	"Montscan/agent"
	"Montscan/config"
	"Montscan/providers"

	ftpserver "github.com/goftp/server"
)

type ScannerDriver struct {
	rootPath string
	agent    *agent.Agent
}

func NewScannerDriver(rootPath string, agent *agent.Agent) *ScannerDriver {
	return &ScannerDriver{
		rootPath: rootPath,
		agent:    agent,
	}
}

func (d *ScannerDriver) realPath(path string) string {
	return filepath.Join(d.rootPath, path)
}

func (d *ScannerDriver) Init(*ftpserver.Conn) {
	log.Println("FTP client connected")
}

func (d *ScannerDriver) Stat(path string) (ftpserver.FileInfo, error) {
	realPath := d.realPath(path)
	info, err := os.Stat(realPath)
	if err != nil {
		return nil, err
	}
	return &FileInfo{info}, nil
}

func (d *ScannerDriver) ChangeDir(path string) error {
	realPath := d.realPath(path)
	info, err := os.Stat(realPath)
	if err != nil {
		return err
	}
	if !info.IsDir() {
		return fmt.Errorf("not a directory: %s", path)
	}
	return nil
}

func (d *ScannerDriver) ListDir(path string, callback func(ftpserver.FileInfo) error) error {
	realPath := d.realPath(path)
	entries, err := os.ReadDir(realPath)
	if err != nil {
		return err
	}
	for _, entry := range entries {
		info, err := entry.Info()
		if err != nil {
			continue
		}
		if err := callback(&FileInfo{info}); err != nil {
			return err
		}
	}
	return nil
}

func (d *ScannerDriver) DeleteDir(path string) error {
	realPath := d.realPath(path)
	return os.RemoveAll(realPath)
}

func (d *ScannerDriver) DeleteFile(path string) error {
	realPath := d.realPath(path)
	return os.Remove(realPath)
}

func (d *ScannerDriver) Rename(from, to string) error {
	toReal := d.realPath(to)
	if _, err := os.Stat(toReal); err == nil {
		return fmt.Errorf("destination already exists: %s", to)
	} else if !os.IsNotExist(err) {
		return err
	}
	return os.Rename(d.realPath(from), toReal)
}

func (d *ScannerDriver) MakeDir(path string) error {
	realPath := d.realPath(path)
	return os.MkdirAll(realPath, 0755)
}

func (d *ScannerDriver) GetFile(path string, offset int64) (int64, io.ReadCloser, error) {
	realPath := d.realPath(path)
	f, err := os.Open(realPath)
	if err != nil {
		return 0, nil, err
	}

	info, err := f.Stat()
	if err != nil {
		_ = f.Close()
		return 0, nil, err
	}

	if offset > 0 {
		if _, err := f.Seek(offset, io.SeekStart); err != nil {
			_ = f.Close()
			return 0, nil, err
		}
	}

	return info.Size() - offset, f, nil
}

func (d *ScannerDriver) PutFile(path string, data io.Reader, appendData bool) (int64, error) {
	realPath := d.realPath(path)

	// Ensure parent directory exists
	dir := filepath.Dir(realPath)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return 0, err
	}

	var f *os.File
	var err error
	if appendData {
		f, err = os.OpenFile(realPath, os.O_WRONLY|os.O_CREATE|os.O_APPEND, 0644)
		if err != nil {
			return 0, err
		}
	} else {
		claimed := false
		for attempt := 0; attempt < providers.MaxNameAttempts; attempt++ {
			candidate := filepath.Join(dir, providers.CandidateName(filepath.Base(realPath), attempt))
			f, err = os.OpenFile(candidate, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0644)
			if err == nil {
				realPath = candidate
				claimed = true
				break
			}
			if !os.IsExist(err) {
				return 0, err
			}
		}
		if !claimed {
			return 0, fmt.Errorf("no free filename found for %s", path)
		}
	}
	defer func() { _ = f.Close() }()

	n, err := io.Copy(f, data)
	if err != nil {
		return n, err
	}

	go d.onFileReceived(realPath)

	return n, nil
}

func (d *ScannerDriver) onFileReceived(filePath string) {
	log.Printf("File received: %s", filePath)

	if !strings.HasSuffix(strings.ToLower(filePath), ".pdf") {
		log.Printf("Skipping non-PDF file: %s", filePath)
		return
	}

	if d.agent != nil {
		d.agent.ProcessDocument(filePath)
	} else {
		log.Println("Document processor not initialized")
	}
}

// FileInfo wraps os.FileInfo to implement ftpserver.FileInfo
type FileInfo struct {
	os.FileInfo
}

// Owner returns the file owner
func (f *FileInfo) Owner() string {
	return "scanner"
}

// Group returns the file group
func (f *FileInfo) Group() string {
	return "scanner"
}

// DriverFactory creates ScannerDriver instances
type DriverFactory struct {
	RootPath string
	Agent    *agent.Agent
}

// NewDriver creates a new driver for a connection
func (f *DriverFactory) NewDriver() (ftpserver.Driver, error) {
	return NewScannerDriver(f.RootPath, f.Agent), nil
}

// Auth implements ftpserver.Auth interface
type Auth struct {
	Username string
	Password string
}

// CheckPasswd validates credentials
func (a *Auth) CheckPasswd(username, password string) (bool, error) {
	return username == a.Username && password == a.Password, nil
}

// StartFTPServer starts the FTP server
func StartFTPServer(cfg *config.Config, ag *agent.Agent) error {
	// Ensure upload directory exists
	uploadPath, err := filepath.Abs(cfg.FTPUploadDir)
	if err != nil {
		return fmt.Errorf("failed to resolve upload path: %w", err)
	}

	if err := os.MkdirAll(uploadPath, 0755); err != nil {
		return fmt.Errorf("failed to create upload directory: %w", err)
	}
	log.Printf("FTP upload directory: %s", uploadPath)

	factory := &DriverFactory{
		RootPath: uploadPath,
		Agent:    ag,
	}

	auth := &Auth{
		Username: cfg.FTPUsername,
		Password: cfg.FTPPassword,
	}

	opts := &ftpserver.ServerOpts{
		Factory:  factory,
		Auth:     auth,
		Hostname: cfg.FTPHost,
		Port:     cfg.FTPPort,
		Logger:   &ftpserver.DiscardLogger{},
	}

	ftp := ftpserver.NewServer(opts)

	log.Printf("🚀 Starting FTP server on %s:%d", cfg.FTPHost, cfg.FTPPort)
	return ftp.ListenAndServe()
}
