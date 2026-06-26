# EFIE Evaluation Dockerfile
# Reproducible environment for EFIE benchmarks and evaluation
#
# Build: docker build -t efie-eval .
# Run:   docker run -v $(pwd)/results:/app/results efie-eval

FROM golang:1.24-alpine AS builder

# Install git for repository cloning
RUN apk add --no-cache git

# Set working directory
WORKDIR /app

# Copy go.mod and go.sum first for dependency caching
COPY go.mod go.sum ./
RUN go mod download

# Copy source code
COPY . .

# Build evaluation tools
RUN go build -o /app/efie-eval ./paper/evaluate

# Runtime stage
FROM alpine:3.19

# Install git and python for analysis
RUN apk add --no-cache git python3 py3-pip

# Install matplotlib for plotting
RUN pip3 install --break-system-packages matplotlib numpy

# Copy built binary and scripts
COPY --from=builder /app/efie-eval /usr/local/bin/efie-eval
COPY paper/evaluate/generate_plots.py /app/scripts/generate_plots.py
COPY paper/evaluate/ablation_study.py /app/scripts/ablation_study.py

WORKDIR /app

# Default command: run full evaluation
CMD ["efie-eval"]
