import logging
import ollama
import requests
import base64
import os

from requests.auth import HTTPBasicAuth
from datetime import datetime
from io import BytesIO
from pdf2image import convert_from_path

logger = logging.getLogger(__name__)

class Agent:
    """
    Handles vision AI analysis, AI naming, and Nextcloud upload of scanned documents.

    This class provides methods to:
    - Extract images from PDF files for vision analysis.
    - Generate descriptive filenames using AI vision models.
    - Upload processed files to Nextcloud via WebDAV.
    """

    def __init__(self):
        """
        Initialize the Agent with environment variables.

        Environment variables:
        - OLLAMA_HOST: Host URL for the Ollama AI service.
        - OLLAMA_MODEL: Vision model name for Ollama AI.
        - NEXTCLOUD_URL: Base URL for Nextcloud.
        - NEXTCLOUD_USERNAME: Username for Nextcloud authentication.
        - NEXTCLOUD_PASSWORD: Password for Nextcloud authentication.
        - NEXTCLOUD_UPLOAD_PATH: Path in Nextcloud to upload files.
        """
        self.ollama_host = os.getenv('OLLAMA_HOST', 'http://localhost:11434')
        self.ollama_model = os.getenv('OLLAMA_MODEL', 'ministral-3:3b-instruct-2512-q4_K_M')
        self.nextcloud_url = os.getenv('NEXTCLOUD_URL')
        self.nextcloud_username = os.getenv('NEXTCLOUD_USERNAME')
        self.nextcloud_password = os.getenv('NEXTCLOUD_PASSWORD')
        self.nextcloud_path = os.getenv('NEXTCLOUD_UPLOAD_PATH', '/Documents/Scanned')

    def extract_image(self, pdf_path: str) -> str | None:
        """
        Extract first page as image from a PDF file.

        Args:
            pdf_path (str): Path to the PDF file.

        Returns:
            str: Base64 UTF-8 encoded image of the first page, or None on failure.
        """
        logger.info(f"Extracting first page from PDF: {pdf_path}")

        try:
            images = convert_from_path(pdf_path, dpi=300)

            # return base64 utf-8 image of the first page
            if images:
                buffered = BytesIO()
                images[0].save(buffered, format="JPEG")
                return base64.b64encode(buffered.getvalue()).decode('utf-8')

        except Exception as e:
            logger.error(f"Error extracting image from PDF: {e}")
            return None

    def generate_filename(self, image: str) -> str:
        """
        Generate a descriptive filename using AI vision model based on document image.

        Args:
            image (str): Base64 encoded image of the document.

        Returns:
            str: AI-generated filename with a .pdf extension.
        """
        logger.info("Generating filename with AI...")

        try:
            prompt = """Based on this scanned document image, generate a concise, descriptive filename (without extension).
The filename should:
- Be 3-6 words maximum
- In french
- Use underscores instead of spaces
- Be descriptive of the document's content
- Use uppercase letters
- Not include special characters except underscores and hyphens
- Include relevant name if mentioned in the document
- Include relevant date if mentioned in the document at the end of the filename (format: DD-MM-YYYY)

Respond with ONLY the filename, nothing else."""

            response = ollama.chat(
                model=self.ollama_model,
                messages=[{'role': 'user', 'content': prompt, 'images': [image]}],
            )

            suggested_name = response['message']['content'].strip()

            filename = f"{suggested_name}.pdf"
            logger.info(f"Agent suggested filename: {filename}")
            return filename

        except Exception as e:
            logger.error(f"Error generating filename with AI: {e}")
            return f"scan_{datetime.now().strftime('%Y%m%d_%H%M%S')}.pdf"

    def upload_to_nextcloud(self, local_path: str, remote_filename: str) -> bool:
        """
        Upload a file to Nextcloud via WebDAV.

        Args:
            local_path (str): Path to the local file.
            remote_filename (str): Filename to use in Nextcloud.

        Returns:
            bool: True if the upload was successful, False otherwise.
        """
        if not self.nextcloud_url or not self.nextcloud_username or not self.nextcloud_password:
            logger.error("Nextcloud credentials not configured")
            return False

        try:
            webdav_base = f"{self.nextcloud_url.rstrip('/')}/remote.php/dav/files/{self.nextcloud_username}"
            remote_path = f"{self.nextcloud_path}/{remote_filename}".replace('//', '/')
            upload_url = f"{webdav_base}{remote_path}"

            logger.info(f"Uploading to Nextcloud: {remote_path}")

            with open(local_path, 'rb') as f:
                file_data = f.read()

            response = requests.put(
                upload_url,
                data=file_data,
                auth=HTTPBasicAuth(self.nextcloud_username, self.nextcloud_password),
                headers={'Content-Type': 'application/pdf'},
                timeout=300,
                verify=False
            )

            if response.status_code in [201, 204]:
                logger.info(f"Successfully uploaded to Nextcloud: {remote_path}")
                return True
            else:
                logger.error(f"Failed to upload to Nextcloud. Status: {response.status_code}, Response: {response.text}")
                return False

        except FileNotFoundError:
            logger.error(f"Local file not found: {local_path}")
            return False
        except requests.exceptions.RequestException as e:
            logger.error(f"Network error uploading to Nextcloud: {e}")
            return False
        except Exception as e:
            logger.error(f"Error uploading to Nextcloud: {e}")
            return False

    def process_document(self, pdf_path: str) -> bool:
        """
        Process a document through the complete pipeline.

        Steps:
        1. Extract first page image from the PDF.
        2. Generate a descriptive filename using AI vision model.
        3. Upload the file to Nextcloud.

        Args:
            pdf_path (str): Path to the PDF file.

        Returns:
            bool: True if the document was processed successfully, False otherwise.
        """
        logger.info(f"Processing document: {pdf_path}")

        try:
            image = self.extract_image(pdf_path)

            new_filename = self.generate_filename(image)

            if self.upload_to_nextcloud(pdf_path, new_filename):
                logger.info(f"Document processing completed successfully: {new_filename}")
                return True
            else:
                logger.error("Failed to upload to Nextcloud")
                return False

        except Exception as e:
            logger.error(f"Error processing document: {e}")
            return False