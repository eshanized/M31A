# ==============================================================================
# M31A — Terminal AI Coding Agent
# ==============================================================================

BINARY      := m31a
MODULE      := github.com/eshanized/M31A
VERSION     ?= $(shell git describe --tags --always --dirty 2>/dev/null || echo "dev")
COMMIT      := $(shell git rev-parse --short HEAD 2>/dev/null || echo "unknown")
DATE        := $(shell date -u '+%Y-%m-%dT%H:%M:%SZ')

# Build flags
LDFLAGS     := -ldflags "-s -w -X main.Version=$(VERSION) -X main.Commit=$(COMMIT) -X main.Date=$(DATE) -X main.GoVersion=$(shell go version | cut -d' ' -f3)"
GOFLAGS     := CGO_ENABLED=0
GO          := go
BUILDFLAGS  := -trimpath

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
	@$(GOFLAGS) $(GO) build $(BUILDFLAGS) $(LDFLAGS) -o $(BINARY) $(CMD_DIR)
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

## run-latest        — Build and run the latest released version
run-latest:
	@printf "\033[0;32m[run-latest]\033[0m Fetching latest release tag...\n"
	@LATEST_TAG=$$(git tag --sort=-v:refname | head -1); \
	if [ -z "$$LATEST_TAG" ]; then \
		printf "\033[0;31m[run-latest]\033[0m No tags found. Run 'git fetch --tags' first.\n"; \
		exit 1; \
	fi; \
	printf "\033[0;32m[run-latest]\033[0m Building from tag $$LATEST_TAG...\n"; \
	CGO_ENABLED=0 go build -trimpath \
		-ldflags "-s -w -X main.Version=$$LATEST_TAG" \
		-o $(BINARY) $(CMD_DIR) && \
	printf "\033[0;32m[run-latest]\033[0m Starting $(BINARY) $$LATEST_TAG...\n"; \
	./$(BINARY)

# ==============================================================================
# Testing
# ==============================================================================

## test              — Run all tests with race detector and coverage
test:
	@printf "\033[0;32m[test]\033[0m Running tests with race detector...\n"
	@$(GO) test -race -cover -timeout 30s -coverprofile=$(COVER_OUT) ./...
	@printf "\033[0;32m[test]\033[0m Coverage report: $(COVER_OUT)\n"

## test-fast         — Run tests without race detector (faster)
test-fast:
	@printf "\033[0;32m[test]\033[0m Running tests (fast mode)...\n"
	@$(GO) test -timeout 30s -cover ./...

## test-verbose      — Run tests with verbose output
test-verbose:
	@printf "\033[0;32m[test]\033[0m Running tests (verbose)...\n"
	@$(GO) test -v -race -cover -timeout 30s ./...

## test-specific     — Run specific test (e.g., make test-specific TEST=TestReplModel)
test-specific:
	@printf "\033[0;32m[test]\033[0m Running: $(TEST)\n"
	@$(GO) test -v -race -timeout 30s -run $(TEST) ./...

## bench             — Run benchmarks
bench:
	@printf "\033[0;32m[bench]\033[0m Running benchmarks...\n"
	@$(GO) test -bench=. -benchmem -run=^$$ ./...

## bench-verbose     — Run benchmarks with verbose output
bench-verbose:
	@printf "\033[0;32m[bench]\033[0m Running benchmarks (verbose)...\n"
	@$(GO) test -v -bench=. -benchmem -run=^$$ ./...

## cover             — Generate HTML coverage report
cover: test
	@printf "\033[0;32m[cover]\033[0m Generating HTML coverage report...\n"
	@$(GO) tool cover -html=$(COVER_OUT) -o $(COVER_HTML)
	@printf "\033[0;32m[cover]\033[0m Report: $(COVER_HTML)\n"

# ==============================================================================
# Code quality
# ==============================================================================

## lint              — Run golangci-lint
lint:
	@printf "\033[0;32m[lint]\033[0m Running golangci-lint...\n"
	@golangci-lint run ./... --timeout=5m

## lint-fix          — Run linter with auto-fix
lint-fix:
	@printf "\033[0;32m[lint]\033[0m Running golangci-lint with auto-fix...\n"
	@golangci-lint run ./... --fix --timeout=5m

## vet               — Run go vet
vet:
	@printf "\033[0;32m[vet]\033[0m Running go vet...\n"
	@$(GO) vet ./...

## fmt               — Format all Go files
fmt:
	@printf "\033[0;32m[fmt]\033[0m Formatting Go files...\n"
	@$(GO) fmt ./...
	@goimports -w $$(find . -name '*.go' -not -path './vendor/*') 2>/dev/null || true

## tidy              — Clean up go.mod dependencies
tidy:
	@printf "\033[0;32m[tidy]\033[0m Running go mod tidy...\n"
	@$(GO) mod tidy

## check             — Run fmt, tidy, vet, lint, test in sequence
check: fmt tidy vet lint test
	@printf "\033[0;32m[check]\033[0m All checks passed!\n"

## validate-release  — Run full release validation suite
validate-release:
	@printf "\033[0;32m[validate]\033[0m Running release validation harness...\n"
	@scripts/validate-release.sh
	@printf "\033[0;32m[validate]\033[0m Done — see VALIDATION_REPORT.md\n"

# ==============================================================================
# Cross-compilation
# ==============================================================================

## cross             — Build binaries for all supported platforms
cross: $(foreach os,$(OS_TARGETS),$(foreach arch,$(ARCH_TARGETS),build-$(os)-$(arch)))

build-linux-amd64:
	@printf "\033[0;32m[cross]\033[0m Building linux/amd64...\n"
	@GOOS=linux GOARCH=amd64 $(GOFLAGS) $(GO) build $(BUILDFLAGS) $(LDFLAGS) -o $(DIST_DIR)/$(BINARY)-linux-amd64 $(CMD_DIR)

build-linux-arm64:
	@printf "\033[0;32m[cross]\033[0m Building linux/arm64...\n"
	@GOOS=linux GOARCH=arm64 $(GOFLAGS) $(GO) build $(BUILDFLAGS) $(LDFLAGS) -o $(DIST_DIR)/$(BINARY)-linux-arm64 $(CMD_DIR)

build-darwin-amd64:
	@printf "\033[0;32m[cross]\033[0m Building darwin/amd64...\n"
	@GOOS=darwin GOARCH=amd64 $(GOFLAGS) $(GO) build $(BUILDFLAGS) $(LDFLAGS) -o $(DIST_DIR)/$(BINARY)-darwin-amd64 $(CMD_DIR)

build-darwin-arm64:
	@printf "\033[0;32m[cross]\033[0m Building darwin/arm64...\n"
	@GOOS=darwin GOARCH=arm64 $(GOFLAGS) $(GO) build $(BUILDFLAGS) $(LDFLAGS) -o $(DIST_DIR)/$(BINARY)-darwin-arm64 $(CMD_DIR)

build-windows-amd64:
	@printf "\033[0;32m[cross]\033[0m Building windows/amd64...\n"
	@GOOS=windows GOARCH=amd64 $(GOFLAGS) $(GO) build $(BUILDFLAGS) $(LDFLAGS) -o $(DIST_DIR)/$(BINARY)-windows-amd64.exe $(CMD_DIR)

build-windows-arm64:
	@printf "\033[0;32m[cross]\033[0m Building windows/arm64...\n"
	@GOOS=windows GOARCH=arm64 $(GOFLAGS) $(GO) build $(BUILDFLAGS) $(LDFLAGS) -o $(DIST_DIR)/$(BINARY)-windows-arm64.exe $(CMD_DIR)

# ==============================================================================
# Release
# ==============================================================================

## release           — Create release via goreleaser (snapshot)
release:
	@printf "\033[0;32m[release]\033[0m Running goreleaser...\n"
	@goreleaser release --snapshot --clean

## release-dry       — Dry run goreleaser
release-dry:
	@printf "\033[0;32m[release]\033[0m Running goreleaser (dry run)...\n"
	@goreleaser release --snapshot --clean --skip=validate --skip=publish

# ==============================================================================
# Utilities
# ==============================================================================

## deps              — Update dependencies
deps:
	@printf "\033[0;32m[deps]\033[0m Updating dependencies...\n"
	@$(GO) get -u ./...
	@$(GO) mod tidy

## deps-verify       — Verify dependency checksums
deps-verify:
	@printf "\033[0;32m[deps]\033[0m Verifying dependencies...\n"
	@$(GO) mod verify

## size              — Show binary size
size: build
	@printf "\033[0;32m[size]\033[0m Binary size:\n"
	@du -h $(BINARY)
	@printf "\033[0;32m[size]\033[0m Stripped size:\n"
	@ls -lh $(BINARY) | awk '{print $$5}'

## clean             — Remove build artifacts
clean:
	@printf "\033[0;33m[clean]\033[0m Removing build artifacts...\n"
	@rm -f $(BINARY) $(BINARY)-debug
	@rm -rf $(DIST_DIR)
	@rm -f $(COVER_OUT) $(COVER_HTML)
	@rm -rf cmd/m31a/cover.out
	@printf "\033[0;33m[clean]\033[0m Done\n"

## nuke              — Clean everything including vendor and cache
nuke: clean
	@printf "\033[0;31m[nuke]\033[0m Removing vendor, cache, and build cache...\n"
	@rm -rf vendor
	@$(GO) clean -cache -modcache -testcache
	@printf "\033[0;31m[nuke]\033[0m Done\n"

## install           — Install binary to GOBIN
install:
	@printf "\033[0;32m[install]\033[0m Installing via go install...\n"
	@CGO_ENABLED=0 $(GO) install $(BUILDFLAGS) $(LDFLAGS) $(CMD_DIR)
	@printf "\033[0;32m[install]\033[0m Done\n"

# ==============================================================================
# Platform-specific installation
# ==============================================================================

INSTALL_PREFIX ?= /usr/local

## install-linux-amd64    — Install linux/amd64 binary to $(INSTALL_PREFIX)/bin
install-linux-amd64: build-linux-amd64
	@printf "\033[0;32m[install]\033[0m Installing linux/amd64 to $(INSTALL_PREFIX)/bin...\n"
	@install -d $(INSTALL_PREFIX)/bin
	@install -m 755 $(DIST_DIR)/$(BINARY)-linux-amd64 $(INSTALL_PREFIX)/bin/$(BINARY)
	@printf "\033[0;32m[install]\033[0m Done: $(INSTALL_PREFIX)/bin/$(BINARY)\n"

## install-linux-arm64    — Install linux/arm64 binary to $(INSTALL_PREFIX)/bin
install-linux-arm64: build-linux-arm64
	@printf "\033[0;32m[install]\033[0m Installing linux/arm64 to $(INSTALL_PREFIX)/bin...\n"
	@install -d $(INSTALL_PREFIX)/bin
	@install -m 755 $(DIST_DIR)/$(BINARY)-linux-arm64 $(INSTALL_PREFIX)/bin/$(BINARY)
	@printf "\033[0;32m[install]\033[0m Done: $(INSTALL_PREFIX)/bin/$(BINARY)\n"

## install-darwin-amd64   — Install darwin/amd64 binary to $(INSTALL_PREFIX)/bin
install-darwin-amd64: build-darwin-amd64
	@printf "\033[0;32m[install]\033[0m Installing darwin/amd64 to $(INSTALL_PREFIX)/bin...\n"
	@install -d $(INSTALL_PREFIX)/bin
	@install -m 755 $(DIST_DIR)/$(BINARY)-darwin-amd64 $(INSTALL_PREFIX)/bin/$(BINARY)
	@printf "\033[0;32m[install]\033[0m Done: $(INSTALL_PREFIX)/bin/$(BINARY)\n"

## install-darwin-arm64   — Install darwin/arm64 binary to $(INSTALL_PREFIX)/bin
install-darwin-arm64: build-darwin-arm64
	@printf "\033[0;32m[install]\033[0m Installing darwin/arm64 to $(INSTALL_PREFIX)/bin...\n"
	@install -d $(INSTALL_PREFIX)/bin
	@install -m 755 $(DIST_DIR)/$(BINARY)-darwin-arm64 $(INSTALL_PREFIX)/bin/$(BINARY)
	@printf "\033[0;32m[install]\033[0m Done: $(INSTALL_PREFIX)/bin/$(BINARY)\n"

## install-windows-amd64  — Install windows/amd64 binary to $(INSTALL_PREFIX)/bin
install-windows-amd64: build-windows-amd64
	@printf "\033[0;32m[install]\033[0m Installing windows/amd64 to $(INSTALL_PREFIX)/bin...\n"
	@install -d $(INSTALL_PREFIX)/bin
	@install -m 755 $(DIST_DIR)/$(BINARY)-windows-amd64.exe $(INSTALL_PREFIX)/bin/$(BINARY).exe
	@printf "\033[0;32m[install]\033[0m Done: $(INSTALL_PREFIX)/bin/$(BINARY).exe\n"

## install-windows-arm64  — Install windows/arm64 binary to $(INSTALL_PREFIX)/bin
install-windows-arm64: build-windows-arm64
	@printf "\033[0;32m[install]\033[0m Installing windows/arm64 to $(INSTALL_PREFIX)/bin...\n"
	@install -d $(INSTALL_PREFIX)/bin
	@install -m 755 $(DIST_DIR)/$(BINARY)-windows-arm64.exe $(INSTALL_PREFIX)/bin/$(BINARY).exe
	@printf "\033[0;32m[install]\033[0m Done: $(INSTALL_PREFIX)/bin/$(BINARY).exe\n"

## install-all           — Build and install all platform binaries
install-all: install-linux-amd64 install-linux-arm64 install-darwin-amd64 install-darwin-arm64 install-windows-amd64 install-windows-arm64
	@printf "\033[0;32m[install]\033[0m All platforms installed to $(INSTALL_PREFIX)/bin\n"

## uninstall            — Remove installed binary from $(INSTALL_PREFIX)/bin
uninstall:
	@printf "\033[0;33m[uninstall]\033[0m Removing $(INSTALL_PREFIX)/bin/$(BINARY)...\n"
	@rm -f $(INSTALL_PREFIX)/bin/$(BINARY) $(INSTALL_PREFIX)/bin/$(BINARY).exe
	@printf "\033[0;33m[uninstall]\033[0m Done\n"

## version           — Show version info
version:
	@echo "Version: $(VERSION)"
	@echo "Commit:  $(COMMIT)"
	@echo "Date:    $(DATE)"
	@echo "Go:      $(shell $(GO) version)"

# ==============================================================================
# Phony targets
# ==============================================================================

.PHONY: build debug dev run-latest help \
        test test-fast test-verbose test-specific \
        bench bench-verbose cover \
        lint lint-fix vet fmt tidy check validate-release \
        cross $(foreach os,$(OS_TARGETS),$(foreach arch,$(ARCH_TARGETS),build-$(os)-$(arch))) \
        release release-dry \
        deps deps-verify size clean nuke install version \
        install-linux-amd64 install-linux-arm64 \
        install-darwin-amd64 install-darwin-arm64 \
        install-windows-amd64 install-windows-arm64 \
        install-all uninstall
