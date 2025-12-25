# ==============================================================================
# M31A — Terminal AI Coding Agent
# ==============================================================================

# Terminal colors
GREEN     := \033[0;32m
YELLOW    := \033[0;33m
RED       := \033[0;31m
NC        := \033[0m

BINARY      := m31a
MODULE      := github.com/eshanized/M31A
VERSION     ?= $(shell git describe --tags --always --dirty 2>/dev/null || echo "dev")
COMMIT      := $(shell git rev-parse --short HEAD 2>/dev/null || echo "unknown")
DATE        := $(shell date -u '+%Y-%m-%dT%H:%M:%SZ')

# Build flags
LDFLAGS     := -ldflags "-s -w -X main.Version=$(VERSION) -X main.Commit=$(COMMIT) -X main.Date=$(DATE)"
GOFLAGS     := CGO_ENABLED=0
GO          := go

# Directories
CMD_DIR     := ./cmd/m31a
DIST_DIR    := dist
COVER_OUT   := coverage.out
COVER_HTML  := coverage.html

# Cross-compilation targets
OS_TARGETS  := linux darwin windows
ARCH_TARGETS := amd64 arm64

# ==============================================================================
# Primary targets
# ==============================================================================

.DEFAULT_GOAL := help

## help              — Show this help
help:
	@printf "\033[1;36mM31A Makefile Targets\033[0m\n"
	@printf "=========================\n\n"
	@awk 'BEGIN {FS = ":.*##"; pattern = "## "} /^## /{cmd=$$1; gsub(/## /, "", cmd); desc=$$2; printf "  \033[1;32m%-20s\033[0m %s\n", cmd, desc}' $(MAKEFILE_LIST)
	@printf "\n"

## build             — Build binary for current platform (optimized)
build:
	@printf "\033[0;32m[build]\033[0m Building $(BINARY) for $(shell go env GOOS)/$(shell go env GOARCH)...\n"
	@$(GOFLAGS) $(GO) build $(LDFLAGS) -o $(BINARY) $(CMD_DIR)
	@printf "\033[0;32m[build]\033[0m Done: $(BINARY) ($$(du -h $(BINARY) | cut -f1))\n"

## debug             — Build with debug symbols (no strip, static)
debug:
	@printf "\033[0;32m[debug]\033[0m Building debug binary...\n"
	@CGO_ENABLED=0 $(GO) build -gcflags "all=-N -l" -o $(BINARY)-debug $(CMD_DIR)
	@printf "\033[0;32m[debug]\033[0m Done: $(BINARY)-debug\n"

## dev               — Build and run immediately
dev: build
	@printf "\033[0;32m[run]\033[0m Starting $(BINARY)...\n"
	@./$(BINARY)

# ==============================================================================
# Testing
# ==============================================================================

## test              — Run all tests with race detector and coverage
test:
	@echo "$(GREEN)[test]$(NC) Running tests with race detector..."
	@$(GO) test -race -cover -coverprofile=$(COVER_OUT) ./...
	@echo "$(GREEN)[test]$(NC) Coverage report: $(COVER_OUT)"

## test-fast         — Run tests without race detector (faster)
test-fast:
	@echo "$(GREEN)[test]$(NC) Running tests (fast mode)..."
	@$(GO) test -cover ./...

## test-verbose      — Run tests with verbose output
test-verbose:
	@echo "$(GREEN)[test]$(NC) Running tests (verbose)..."
	@$(GO) test -v -race -cover ./...

## test-specific     — Run specific test (e.g., make test-specific TEST=TestReplModel)
test-specific:
	@echo "$(GREEN)[test]$(NC) Running: $(TEST)"
	@$(GO) test -v -race -run $(TEST) ./...

## bench             — Run benchmarks
bench:
	@echo "$(GREEN)[bench]$(NC) Running benchmarks..."
	@$(GO) test -bench=. -benchmem -run=^$$ ./...

## bench-verbose     — Run benchmarks with verbose output
bench-verbose:
	@echo "$(GREEN)[bench]$(NC) Running benchmarks (verbose)..."
	@$(GO) test -v -bench=. -benchmem -run=^$$ ./...

## cover             — Generate HTML coverage report
cover: test
	@echo "$(GREEN)[cover]$(NC) Generating HTML coverage report..."
	@$(GO) tool cover -html=$(COVER_OUT) -o $(COVER_HTML)
	@echo "$(GREEN)[cover]$(NC) Report: $(COVER_HTML)"

# ==============================================================================
# Code quality
# ==============================================================================

## lint              — Run golangci-lint
lint:
	@echo "$(GREEN)[lint]$(NC) Running golangci-lint..."
	@golangci-lint run ./... --timeout=5m

## lint-fix          — Run linter with auto-fix
lint-fix:
	@echo "$(GREEN)[lint]$(NC) Running golangci-lint with auto-fix..."
	@golangci-lint run ./... --fix --timeout=5m

## vet               — Run go vet
vet:
	@echo "$(GREEN)[vet]$(NC) Running go vet..."
	@$(GO) vet ./...

## fmt               — Format all Go files
fmt:
	@echo "$(GREEN)[fmt]$(NC) Formatting Go files..."
	@$(GO) fmt ./...
	@goimports -w $$(find . -name '*.go' -not -path './vendor/*') 2>/dev/null || true

## tidy              — Clean up go.mod dependencies
tidy:
	@echo "$(GREEN)[tidy]$(NC) Running go mod tidy..."
	@$(GO) mod tidy

## check             — Run fmt, vet, lint, test in sequence
check: fmt tidy vet test
	@echo "$(GREEN)[check]$(NC) All checks passed!"

# ==============================================================================
# Cross-compilation
# ==============================================================================

## cross             — Build binaries for all supported platforms
cross: $(foreach os,$(OS_TARGETS),$(foreach arch,$(ARCH_TARGETS),build-$(os)-$(arch)))

build-linux-amd64:
	@echo "$(GREEN)[cross]$(NC) Building linux/amd64..."
	@GOOS=linux GOARCH=amd64 $(GOFLAGS) $(GO) build $(LDFLAGS) -o $(DIST_DIR)/$(BINARY)-linux-amd64 $(CMD_DIR)

build-linux-arm64:
	@echo "$(GREEN)[cross]$(NC) Building linux/arm64..."
	@GOOS=linux GOARCH=arm64 $(GOFLAGS) $(GO) build $(LDFLAGS) -o $(DIST_DIR)/$(BINARY)-linux-arm64 $(CMD_DIR)

build-darwin-amd64:
	@echo "$(GREEN)[cross]$(NC) Building darwin/amd64..."
	@GOOS=darwin GOARCH=amd64 $(GOFLAGS) $(GO) build $(LDFLAGS) -o $(DIST_DIR)/$(BINARY)-darwin-amd64 $(CMD_DIR)

build-darwin-arm64:
	@echo "$(GREEN)[cross]$(NC) Building darwin/arm64..."
	@GOOS=darwin GOARCH=arm64 $(GOFLAGS) $(GO) build $(LDFLAGS) -o $(DIST_DIR)/$(BINARY)-darwin-arm64 $(CMD_DIR)

build-windows-amd64:
	@echo "$(GREEN)[cross]$(NC) Building windows/amd64..."
	@GOOS=windows GOARCH=amd64 $(GOFLAGS) $(GO) build $(LDFLAGS) -o $(DIST_DIR)/$(BINARY)-windows-amd64.exe $(CMD_DIR)

build-windows-arm64:
	@echo "$(GREEN)[cross]$(NC) Building windows/arm64..."
	@GOOS=windows GOARCH=arm64 $(GOFLAGS) $(GO) build $(LDFLAGS) -o $(DIST_DIR)/$(BINARY)-windows-arm64.exe $(CMD_DIR)

# ==============================================================================
# Release
# ==============================================================================

## release           — Create release via goreleaser (snapshot)
release:
	@echo "$(GREEN)[release]$(NC) Running goreleaser..."
	@goreleaser release --snapshot --clean

## release-dry       — Dry run goreleaser
release-dry:
	@echo "$(GREEN)[release]$(NC) Running goreleaser (dry run)..."
	@goreleaser release --snapshot --clean --skip=validate --skip=publish

# ==============================================================================
# Utilities
# ==============================================================================

## deps              — Update dependencies
deps:
	@echo "$(GREEN)[deps]$(NC) Updating dependencies..."
	@$(GO) get -u ./...
	@$(GO) mod tidy

## deps-verify       — Verify dependency checksums
deps-verify:
	@echo "$(GREEN)[deps]$(NC) Verifying dependencies..."
	@$(GO) mod verify

## size              — Show binary size
size: build
	@echo "$(GREEN)[size]$(NC) Binary size:"
	@du -h $(BINARY)
	@echo "$(GREEN)[size]$(NC) Stripped size:"
	@ls -lh $(BINARY) | awk '{print $$5}'

## clean             — Remove build artifacts
clean:
	@echo "$(YELLOW)[clean]$(NC) Removing build artifacts..."
	@rm -f $(BINARY) $(BINARY)-debug
	@rm -rf $(DIST_DIR)
	@rm -f $(COVER_OUT) $(COVER_HTML)
	@rm -rf cmd/m31a/cover.out
	@echo "$(YELLOW)[clean]$(NC) Done"

## nuke              — Clean everything including vendor and cache
nuke: clean
	@echo "$(RED)[nuke]$(NC) Removing vendor, cache, and build cache..."
	@rm -rf vendor
	@$(GO) clean -cache -modcache -testcache
	@echo "$(RED)[nuke]$(NC) Done"

## install           — Install binary to GOBIN
install:
	@echo "$(GREEN)[install]$(NC) Installing via go install..."
	@CGO_ENABLED=0 $(GO) install $(LDFLAGS) $(CMD_DIR)
	@echo "$(GREEN)[install]$(NC) Done"

## version           — Show version info
version:
	@echo "Version: $(VERSION)"
	@echo "Commit:  $(COMMIT)"
	@echo "Date:    $(DATE)"
	@echo "Go:      $(shell $(GO) version)"

# ==============================================================================
# Phony targets
# ==============================================================================

.PHONY: build debug dev help \
        test test-fast test-verbose test-specific \
        bench bench-verbose cover \
        lint lint-fix vet fmt tidy check \
        cross $(foreach os,$(OS_TARGETS),$(foreach arch,$(ARCH_TARGETS),build-$(os)-$(arch))) \
        release release-dry \
        deps deps-verify size clean nuke install version
