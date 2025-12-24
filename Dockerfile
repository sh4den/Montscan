# Use Python 3.14 slim image as base
FROM python:3.14-slim

WORKDIR /app

RUN apt-get update && apt-get install -y \
    libpoppler-dev \
    wget \
    && rm -rf /var/lib/apt/lists/*

COPY pyproject.toml uv.lock ./
COPY src/ ./src/

RUN pip install --no-cache-dir uv

RUN uv sync --frozen

# Create scans directory
RUN mkdir -p /app/scans

EXPOSE 21

# Set environment variables
ENV PYTHONUNBUFFERED=1

# Run the application
CMD ["uv", "run", "python", "src/main.py"]

