import os
import logging

from pathlib import Path
from pyftpdlib.authorizers import DummyAuthorizer
from pyftpdlib.servers import FTPServer
from colorama import Fore, Style, init

from services.Agent import Agent
from services.ScannerHandler import ScannerHandler

# Initialize colorama for Windows compatibility
init(autoreset=True)

logging.basicConfig(
    level=logging.INFO,
    format='[%(asctime)s] [%(levelname)s] - %(name)s - %(message)s'
)
logger = logging.getLogger(__name__)


def setup_ftp_server():
    """
    Setup and configure the FTP server.

    This function initializes the FTP server with the following:
    - Host and port configuration from environment variables.
    - User authentication using `DummyAuthorizer`.
    - Upload directory setup.
    - Custom FTP handler (`ScannerHandler`) for processing uploaded files.

    Returns:
        FTPServer: Configured FTP server instance.
    """
    ftp_host = os.getenv('FTP_HOST', '0.0.0.0')  # Default host is 0.0.0.0
    ftp_port = int(os.getenv('FTP_PORT', '21'))  # Default port is 21
    ftp_username = os.getenv('FTP_USERNAME', 'scanner')  # Default username
    ftp_password = os.getenv('FTP_PASSWORD', 'scanner123')  # Default password
    ftp_upload_dir = os.getenv('FTP_UPLOAD_DIR', './scans')  # Default upload directory

    upload_path = Path(ftp_upload_dir).resolve()
    upload_path.mkdir(parents=True, exist_ok=True)
    logger.info(f"FTP upload directory: {upload_path}")

    authorizer = DummyAuthorizer()
    authorizer.add_user(
        ftp_username,
        ftp_password,
        str(upload_path),
        perm='elradfmw'
    )

    handler = ScannerHandler
    handler.authorizer = authorizer
    handler.banner = "Montscan FTP Server Ready"

    handler.agent = Agent()

    server = FTPServer((ftp_host, ftp_port), handler)
    server.max_cons = 256
    server.max_cons_per_ip = 5

    return server


def print_startup_banner():
    """Print a colorful startup banner with configuration details."""
    print(f"\n{Fore.CYAN}{'═' * 70}{Style.RESET_ALL}")
    print(f"{Fore.CYAN}║{Style.RESET_ALL}{Fore.YELLOW}  🖨️  MONTSCAN - Scanner Document Processing System  📄{Style.RESET_ALL}  {Fore.CYAN}║{Style.RESET_ALL}")
    print(f"{Fore.CYAN}{'═' * 70}{Style.RESET_ALL}\n")

    # FTP Configuration
    print(f"{Fore.GREEN}📡 FTP Server Configuration:{Style.RESET_ALL}")
    ftp_host = os.getenv('FTP_HOST', '0.0.0.0')
    ftp_port = os.getenv('FTP_PORT', '21')
    ftp_username = os.getenv('FTP_USERNAME', 'scanner')
    ftp_upload_dir = os.getenv('FTP_UPLOAD_DIR', './scans')

    print(f"   {Fore.WHITE}├─{Style.RESET_ALL} Host: {Fore.CYAN}{ftp_host}{Style.RESET_ALL}")
    print(f"   {Fore.WHITE}├─{Style.RESET_ALL} Port: {Fore.CYAN}{ftp_port}{Style.RESET_ALL}")
    print(f"   {Fore.WHITE}├─{Style.RESET_ALL} Username: {Fore.CYAN}{ftp_username}{Style.RESET_ALL}")
    print(f"   {Fore.WHITE}└─{Style.RESET_ALL} Upload Directory: {Fore.CYAN}{Path(ftp_upload_dir).resolve()}{Style.RESET_ALL}\n")

    # Nextcloud Configuration
    nextcloud_url = os.getenv('NEXTCLOUD_URL')
    if nextcloud_url:
        print(f"{Fore.GREEN}☁️  Nextcloud Integration:{Style.RESET_ALL}")
        print(f"   {Fore.WHITE}└─{Style.RESET_ALL} URL: {Fore.CYAN}{nextcloud_url}{Style.RESET_ALL}\n")
    else:
        print(f"{Fore.YELLOW}⚠️  Nextcloud Integration:{Style.RESET_ALL}")
        print(f"   {Fore.WHITE}└─{Style.RESET_ALL} {Fore.YELLOW}Not configured (NEXTCLOUD_URL not set){Style.RESET_ALL}\n")

    # Ollama Configuration
    ollama_host = os.getenv('OLLAMA_HOST', 'http://localhost:11434')
    print(f"{Fore.GREEN}🤖 AI Processing (Ollama):{Style.RESET_ALL}")
    print(f"   {Fore.WHITE}└─{Style.RESET_ALL} Host: {Fore.CYAN}{ollama_host}{Style.RESET_ALL}\n")

    # Status
    print(f"{Fore.CYAN}{'─' * 70}{Style.RESET_ALL}")
    print(f"{Fore.GREEN}✅ All systems initialized - Ready to process documents!{Style.RESET_ALL}")
    print(f"{Fore.CYAN}{'─' * 70}{Style.RESET_ALL}\n")


def main():
    """
    Main entry point of the application.

    This function:
    - Logs the application startup information.
    - Checks for required environment variables.
    - Sets up and starts the FTP server.
    """
    print_startup_banner()

    try:
        server = setup_ftp_server()

        ftp_host = os.getenv('FTP_HOST', '0.0.0.0')
        ftp_port = int(os.getenv('FTP_PORT', '21'))

        logger.info(f"🚀 Starting FTP server on {ftp_host}:{ftp_port}")
        print(f"{Fore.GREEN}🚀 Server is now running! Press Ctrl+C to stop.{Style.RESET_ALL}\n")

        server.serve_forever()
    except KeyboardInterrupt:
        print(f"\n{Fore.YELLOW}⏹️  Shutting down server...{Style.RESET_ALL}")
        logger.info("Server stopped by user")
    except Exception as e:
        print(f"\n{Fore.RED}❌ Error starting server: {e}{Style.RESET_ALL}")
        logger.error(f"Error starting server: {e}")
        raise


if __name__ == "__main__":
    main()