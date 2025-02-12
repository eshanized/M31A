BINARY  := m31a
MODULE  := github.com/eshanized/M31A
VERSION := $(shell git describe --tags --always --dirty 2>/dev/null || echo "dev")
LDFLAGS := -ldflags "-X main.Version=$(VERSION) -s -w"

.PHONY: build test lint vet clean release

build:
	CGO_ENABLED=0 go build $(LDFLAGS) -o $(BINARY) ./cmd/m31a

test:
	go test -race -cover -coverprofile=coverage.out ./...

lint:
	golangci-lint run ./...

vet:
	go vet ./...

clean:
	rm -f $(BINARY) coverage.out

release:
	goreleaser release --snapshot --clean

.DEFAULT_GOAL := build
