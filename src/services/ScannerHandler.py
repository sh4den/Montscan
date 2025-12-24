import logging
import os

from pyftpdlib.handlers import FTPHandler

logger = logging.getLogger(__name__)

class ScannerHandler(FTPHandler):
    """
    Custom FTP handler for processing uploaded PDF files.

    This class extends the `pyftpdlib.handlers.FTPHandler` to add functionality
    for handling uploaded files. Specifically, it processes PDF files using a
    document processor and deletes them after successful processing.
    """

    agent = None

    def on_file_received(self, file_path):
        """
        Called when a file upload is completed.

        This method is triggered after a file is successfully uploaded to the FTP server.
        It checks if the file is a PDF, processes it using the document processor, and
        deletes the file if processing is successful.

        Args:
            file_path (str): The path to the uploaded file.
        """
        logger.info(f"File received: {file_path}")

        if not file_path.lower().endswith('.pdf'):
            logger.warning(f"Skipping non-PDF file: {file_path}")
            return

        try:
            if self.agent:
                success = self.agent.process_document(file_path)

                if success:
                    try:
                        os.remove(file_path)
                        logger.info(f"Deleted processed file: {file_path}")
                    except Exception as e:
                        logger.error(f"Error deleting file: {e}")
                else:
                    logger.error(f"Failed to process document: {file_path}")
            else:
                logger.error("Document processor not initialized")

        except Exception as e:
            logger.error(f"Error in file received handler: {e}")