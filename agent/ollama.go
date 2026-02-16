package agent

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"strings"
	"time"
)

type OllamaChatRequest struct {
	Model    string          `json:"model"`
	Messages []OllamaMessage `json:"messages"`
	Stream   bool            `json:"stream"`
}

type OllamaMessage struct {
	Role    string   `json:"role"`
	Content string   `json:"content"`
	Images  []string `json:"images,omitempty"`
}

type OllamaChatResponse struct {
	Message struct {
		Content string `json:"content"`
	} `json:"message"`
}

const filenamePromptTemplate = `Based on this scanned document image, generate a concise, descriptive filename (without extension).
The filename should:
- Be 3-6 words maximum
- In %s
- Use underscores instead of spaces
- Be descriptive of the document's content
- Use uppercase letters
- Not include special characters except underscores and hyphens
- Include relevant name if mentioned in the document
- Include relevant date if mentioned in the document at the end of the filename (format: dd-mm-yyyy)

Example: INVOICE_AMAZON_JOHN_2023-11-15

Respond with ONLY the filename, nothing else.`

func (a *Agent) GenerateFilename(image string) string {
	log.Printf("Generating filename with AI...")

	filenamePrompt := fmt.Sprintf(filenamePromptTemplate, a.config.PromptLanguage)

	req := OllamaChatRequest{
		Model: a.config.OllamaModel,
		Messages: []OllamaMessage{
			{
				Role:    "user",
				Content: filenamePrompt,
				Images:  []string{image},
			},
		},
		Stream: false,
	}

	jsonData, err := json.Marshal(req)
	if err != nil {
		log.Printf("Error marshaling request: %v", err)
		return a.fallbackFilename()
	}

	url := fmt.Sprintf("%s/api/chat", strings.TrimSuffix(a.config.OllamaHost, "/"))
	client := &http.Client{Timeout: 120 * time.Second}

	resp, err := client.Post(url, "application/json", bytes.NewBuffer(jsonData))
	if err != nil {
		log.Printf("Error calling Ollama: %v", err)
		return a.fallbackFilename()
	}

	defer func(Body io.ReadCloser) {
		err := Body.Close()
		if err != nil {

		}
	}(resp.Body)

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		log.Printf("Ollama returned status %d: %s", resp.StatusCode, string(body))
		return a.fallbackFilename()
	}

	var result OllamaChatResponse
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		log.Printf("Error decoding Ollama response: %v", err)
		return a.fallbackFilename()
	}

	suggestedName := strings.TrimSpace(result.Message.Content)
	if suggestedName == "" {
		return a.fallbackFilename()
	}

	suggestedName = sanitizeFilename(suggestedName)
	filename := suggestedName + ".pdf"

	log.Printf("Agent suggested filename: %s", filename)
	return filename
}

func (a *Agent) fallbackFilename() string {
	return fmt.Sprintf("scan_%s.pdf", time.Now().Format("20060102_150405"))
}

func sanitizeFilename(name string) string {
	name = strings.ReplaceAll(name, "/", "_")
	name = strings.ReplaceAll(name, "\\", "_")
	name = strings.ReplaceAll(name, ":", "_")
	name = strings.ReplaceAll(name, "*", "_")
	name = strings.ReplaceAll(name, "?", "_")
	name = strings.ReplaceAll(name, "\"", "_")
	name = strings.ReplaceAll(name, "<", "_")
	name = strings.ReplaceAll(name, ">", "_")
	name = strings.ReplaceAll(name, "|", "_")

	name = strings.TrimSuffix(name, ".pdf")
	name = strings.TrimSuffix(name, ".PDF")

	return name
}
