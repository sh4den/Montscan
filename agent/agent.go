package agent

import (
	"log"
	"os"

	"Montscan/config"
	"Montscan/providers"
)

type Agent struct {
	config *config.Config
}

func New(cfg *config.Config) *Agent {
	return &Agent{config: cfg}
}

func (a *Agent) ProcessDocument(pdfPath string) bool {
	log.Printf("Processing document: %s", pdfPath)

	image, err := a.ExtractImage(pdfPath)
	if err != nil {
		log.Printf("Error extracting image from PDF: %v", err)
		return false
	}

	newFilename := a.GenerateFilename(image)

	if a.config.WebDAVEnabled {
		if err := providers.UploadToWebDAV(a.config, pdfPath, newFilename); err != nil {
			log.Printf("Failed to upload to WebDAV: %v", err)
			return false
		}
	} else if a.config.SambaEnabled {
		if err := providers.UploadToSamba(a.config, pdfPath, newFilename); err != nil {
			log.Printf("Failed to upload to Samba: %v", err)
			return false
		}
	} else {
		log.Printf("No upload provider configured, skipping upload for: %s", newFilename)
	}

	if err := os.Remove(pdfPath); err != nil {
		log.Printf("Error deleting file: %v", err)
	} else {
		log.Printf("Deleted processed file: %s", pdfPath)
	}

	log.Printf("Document processing completed successfully: %s", newFilename)
	return true
}
