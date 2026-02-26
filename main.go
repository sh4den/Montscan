package main

import (
	"fmt"
	"log"
	"os"
	"os/signal"
	"path/filepath"
	"syscall"

	"github.com/fatih/color"

	"Montscan/agent"
	"Montscan/config"
	"Montscan/server"
)

func printBanner(cfg *config.Config) {
	cyan := color.New(color.FgCyan).SprintFunc()
	yellow := color.New(color.FgYellow).SprintFunc()
	green := color.New(color.FgGreen).SprintFunc()
	white := color.New(color.FgWhite).SprintFunc()

	fmt.Println(green("📡 FTP Server Configuration:"))
	uploadPath, _ := filepath.Abs(cfg.FTPUploadDir)
	fmt.Printf("   %s├─%s Host: %s\n", white(""), white(""), cyan(cfg.FTPHost))
	fmt.Printf("   %s├─%s Port: %s\n", white(""), white(""), cyan(fmt.Sprintf("%d", cfg.FTPPort)))
	fmt.Printf("   %s├─%s Username: %s\n", white(""), white(""), cyan(cfg.FTPUsername))
	fmt.Printf("   %s└─%s Upload Directory: %s\n", white(""), white(""), cyan(uploadPath))
	fmt.Println()

	if cfg.WebDAVEnabled {
		fmt.Println(green("☁️  WebDAV Provider:"))
		fmt.Printf("   %s└─%s URL: %s\n", white(""), white(""), cyan(cfg.WebDAVURL))
	} else {
		fmt.Println(yellow("⚠️  WebDAV Provider:"))
		fmt.Printf("   %s└─%s %s\n", white(""), white(""), yellow("Not configured (WEBDAV_URL not set)"))
	}
	fmt.Println()

	if cfg.SambaEnabled {
		fmt.Println(green("🗂️  Samba Provider:"))
		fmt.Printf("   %s├─%s Host: %s\n", white(""), white(""), cyan(cfg.SambaHost))
		fmt.Printf("   %s├─%s Share: %s\n", white(""), white(""), cyan(cfg.SambaShare))
		fmt.Printf("   %s├─%s Username: %s\n", white(""), white(""), cyan(cfg.SambaUsername))
		fmt.Printf("   %s└─%s Path: %s\n", white(""), white(""), cyan(cfg.SambaPath))
	} else {
		fmt.Println(yellow("⚠️  Samba Provider:"))
		fmt.Printf("   %s└─%s %s\n", white(""), white(""), yellow("Not configured (SAMBA_HOST or SAMBA_SHARE not set)"))
	}
	fmt.Println()

	fmt.Println(green("🤖 AI Processing (Ollama):"))
	fmt.Printf("   %s├─%s Host: %s\n", white(""), white(""), cyan(cfg.OllamaHost))
	fmt.Printf("   %s└─%s Model: %s\n", white(""), white(""), cyan(cfg.OllamaModel))
	fmt.Println()

	pdfTool := agent.CheckPDFTools()
	if pdfTool != "" {
		fmt.Println(green("📄 PDF Processing:"))
		fmt.Printf("   %s└─%s Tool: %s\n", white(""), white(""), cyan(pdfTool))
	} else {
		fmt.Println(color.New(color.FgRed).Sprint("❌ PDF Processing:"))
		fmt.Println(agent.GetPDFToolInstallInstructions())
	}
	fmt.Println()

	fmt.Println(cyan("──────────────────────────────────────────────────────────────────────"))
	fmt.Println(green("✅ All systems initialized - Ready to process documents!"))
	fmt.Println(cyan("──────────────────────────────────────────────────────────────────────"))
	fmt.Println()
}

func main() {
	log.SetFlags(log.Ldate | log.Ltime | log.Lmsgprefix)
	log.SetPrefix("[Montscan] ")

	cfg := config.Load()

	printBanner(cfg)

	if agent.CheckPDFTools() == "" {
		panic("PDF processing tools not found. Please install one of the supported tools (e.g., pdftotext, pdfinfo) and ensure it's in your system PATH.")
	}

	ag := agent.New(cfg)

	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, syscall.SIGINT, syscall.SIGTERM)

	go func() {
		<-sigChan
		fmt.Println()
		color.Yellow("⏹️  Shutting down server...")
		log.Println("Server stopped by user")
		os.Exit(0)
	}()

	fmt.Println(color.GreenString("🚀 Server is now running! Press Ctrl+C to stop."))
	fmt.Println()

	if err := server.StartFTPServer(cfg, ag); err != nil {
		color.Red("❌ Error starting server: %v", err)
		log.Fatalf("Error starting server: %v", err)
	}
}
