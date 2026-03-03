package providers

import (
	"Montscan/config"
	"fmt"
	"net"
	"os"

	"github.com/hirochachacha/go-smb2"
)

func UploadToSamba(cfg *config.Config, localPath, remoteFilename string) error {
	if cfg.SambaHost == "" || cfg.SambaUsername == "" || cfg.SambaPassword == "" || cfg.SambaShare == "" || cfg.SambaPath == "" {
		return fmt.Errorf("samba configuration is incomplete")
	}

	con, err := net.Dial("tcp", cfg.SambaHost+":445")
	if err != nil {
		return fmt.Errorf("failed to connect to SMB server: %v", err)
	}

	client := &smb2.Dialer{
		Initiator: &smb2.NTLMInitiator{
			User:     cfg.SambaUsername,
			Password: cfg.SambaPassword,
		},
	}

	smbConn, err := client.Dial(con)
	if err != nil {
		return fmt.Errorf("failed to connect to SMB server: %v", err)
	}

	fs, err := smbConn.Mount(cfg.SambaShare)
	if err != nil {
		return fmt.Errorf("failed to mount smbfs: %v", err)
	}

	defer func(fs *smb2.Share) {
		err := fs.Umount()
		if err != nil {
			fmt.Printf("Warning: failed to unmount smbfs: %v\n", err)
		}
	}(fs)

	localFile, err := os.Open(localPath)
	if err != nil {
		return fmt.Errorf("failed to open local file: %v", err)
	}

	defer func(localFile *os.File) {
		err := localFile.Close()
		if err != nil {
			fmt.Printf("Warning: failed to close local file: %v\n", err)
		}
	}(localFile)

	remotePath := cfg.SambaPath + "/" + remoteFilename
	remoteFile, err := fs.Create(remotePath)
	if err != nil {
		return fmt.Errorf("failed to create remote file: %v", err)
	}

	defer func(remoteFile *smb2.File) {
		err := remoteFile.Close()
		if err != nil {
			fmt.Printf("Warning: failed to close remote file: %v\n", err)
		}
	}(remoteFile)

	_, err = localFile.Seek(0, 0)
	if err != nil {
		return fmt.Errorf("failed to seek local file: %v", err)
	}

	_, err = remoteFile.Seek(0, 0)
	if err != nil {
		return fmt.Errorf("failed to seek remote file: %v", err)
	}

	_, err = remoteFile.ReadFrom(localFile)
	if err != nil {
		return fmt.Errorf("failed to write to remote file: %v", err)
	}

	fmt.Printf("Successfully uploaded to Samba: %s\n", remotePath)
	return nil
}
