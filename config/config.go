package config

import (
	"os"
	"strconv"
)

type Config struct {
	// FTP Server settings
	FTPEnabled   bool
	FTPHost      string
	FTPPort      int
	FTPUsername  string
	FTPPassword  string
	FTPUploadDir string

	// WebDAV Settings
	WebDAVEnabled  bool
	WebDAVURL      string
	WebDAVUsername string
	WebDAVPassword string
	WebDAVPath     string
	WebDAVInsecure bool

	// Samba provider settings (egress upload target)
	SambaEnabled  bool
	SambaHost     string
	SambaPort     int
	SambaShare    string
	SambaUsername string
	SambaPassword string
	SambaPath     string

	// Samba server settings (incoming scan source)
	SambaServerEnabled         bool
	SambaServerHost            string
	SambaServerPort            int
	SambaServerShare           string
	SambaServerUsername        string
	SambaServerPassword        string
	SambaServerPath            string
	SambaServerPollIntervalSec int
	SambaServerDeleteAfterRead bool
	SambaServerWorkDir         string

	// OLLAMA Settings
	OllamaHost  string
	OllamaModel string
	Language    string
}

func Load() *Config {
	ftpPort := getEnvInt("FTP_PORT", 21)
	sambaPort := getEnvInt("SAMBA_PORT", 445)
	sambaServerPort := getEnvIntFallback("SAMBA_SERVER_PORT", "SAMBA_INGRESS_PORT", 445)
	sambaServerPoll := getEnvIntFallback("SAMBA_SERVER_POLL_INTERVAL_SEC", "SAMBA_INGRESS_POLL_INTERVAL_SEC", 10)
	if sambaServerPoll < 1 {
		sambaServerPoll = 10
	}

	ftpUploadDir := getEnv("FTP_UPLOAD_DIR", "./scans")

	return &Config{
		FTPEnabled:   getEnv("FTP_ENABLED", "true") == "true",
		FTPHost:      getEnv("FTP_HOST", "0.0.0.0"),
		FTPPort:      ftpPort,
		FTPUsername:  getEnv("FTP_USERNAME", "scanner"),
		FTPPassword:  getEnv("FTP_PASSWORD", "scanner123"),
		FTPUploadDir: ftpUploadDir,

		WebDAVEnabled:  getEnv("WEBDAV_ENABLED", "false") == "true",
		WebDAVURL:      os.Getenv("WEBDAV_URL"),
		WebDAVUsername: os.Getenv("WEBDAV_USERNAME"),
		WebDAVPassword: os.Getenv("WEBDAV_PASSWORD"),
		WebDAVPath:     getEnv("WEBDAV_UPLOAD_PATH", "/Documents/Scanned"),
		WebDAVInsecure: getEnv("WEBDAV_INSECURE", "false") == "true",

		SambaEnabled:  getEnv("SAMBA_ENABLED", "false") == "true",
		SambaHost:     getEnv("SAMBA_HOST", "localhost"),
		SambaPort:     sambaPort,
		SambaShare:    getEnv("SAMBA_SHARE", "scans"),
		SambaUsername: os.Getenv("SAMBA_USERNAME"),
		SambaPassword: os.Getenv("SAMBA_PASSWORD"),
		SambaPath:     getEnv("SAMBA_PATH", "scans"),

		SambaServerEnabled:         getEnvFallback("SAMBA_SERVER_ENABLED", "SAMBA_INGRESS_ENABLED", "false") == "true",
		SambaServerHost:            getEnvFallback("SAMBA_SERVER_HOST", "SAMBA_INGRESS_HOST", "localhost"),
		SambaServerPort:            sambaServerPort,
		SambaServerShare:           getEnvFallback("SAMBA_SERVER_SHARE", "SAMBA_INGRESS_SHARE", "scans"),
		SambaServerUsername:        getEnvWithFallback("SAMBA_SERVER_USERNAME", "SAMBA_INGRESS_USERNAME"),
		SambaServerPassword:        getEnvWithFallback("SAMBA_SERVER_PASSWORD", "SAMBA_INGRESS_PASSWORD"),
		SambaServerPath:            getEnvFallback("SAMBA_SERVER_PATH", "SAMBA_INGRESS_PATH", "/scans"),
		SambaServerPollIntervalSec: sambaServerPoll,
		SambaServerDeleteAfterRead: getEnvFallback("SAMBA_SERVER_DELETE_AFTER_READ", "SAMBA_INGRESS_DELETE_AFTER_READ", "true") == "true",
		SambaServerWorkDir:         getEnvFallback("SAMBA_SERVER_WORK_DIR", "SAMBA_INGRESS_WORK_DIR", ftpUploadDir),

		OllamaHost:  getEnv("OLLAMA_HOST", "http://localhost:11434"),
		OllamaModel: getEnv("OLLAMA_MODEL", "llava"),
		Language:    getEnv("LANGUAGE", "english"),
	}
}

func getEnv(key, defaultValue string) string {
	if value := os.Getenv(key); value != "" {
		return value
	}
	return defaultValue
}

func getEnvWithFallback(primary, fallback string) string {
	if value := os.Getenv(primary); value != "" {
		return value
	}
	return os.Getenv(fallback)
}

func getEnvFallback(primary, fallback, defaultValue string) string {
	if value := os.Getenv(primary); value != "" {
		return value
	}
	if value := os.Getenv(fallback); value != "" {
		return value
	}
	return defaultValue
}

func getEnvInt(key string, defaultValue int) int {
	raw := getEnv(key, strconv.Itoa(defaultValue))
	parsed, err := strconv.Atoi(raw)
	if err != nil {
		return defaultValue
	}
	return parsed
}

func getEnvIntFallback(primary, fallback string, defaultValue int) int {
	raw := getEnvFallback(primary, fallback, strconv.Itoa(defaultValue))
	parsed, err := strconv.Atoi(raw)
	if err != nil {
		return defaultValue
	}
	return parsed
}
