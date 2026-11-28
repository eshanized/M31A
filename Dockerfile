# ---- Stage 1: Builder ----
FROM docker.io/library/node:22-slim AS builder

# Install git (needed by generate-git-commit-info.js script)
RUN apt-get update && apt-get install -y --no-install-recommends git \
  && apt-get clean && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy only package.json files first for better layer caching
# Dependencies only re-install when package files change, not source files
COPY package*.json ./
COPY packages/cli/package*.json ./packages/cli/
COPY packages/core/package*.json ./packages/core/
COPY packages/devtools/package*.json ./packages/devtools/
COPY packages/genai/package*.json ./packages/genai/
COPY packages/test-utils/package*.json ./packages/test-utils/

# Use npm ci for consistent, reliable builds (respects package-lock.json)
RUN HUSKY=0 npm ci --ignore-scripts

# Now copy the rest of the source (after install for better caching)
COPY packages/ ./packages/
COPY tsconfig*.json ./
COPY eslint.config.js ./
COPY scripts/ ./scripts/
COPY esbuild.config.js ./
COPY README.md LICENSE ./

# Pass git commit hash as build arg instead of copying entire .git directory
ARG GIT_COMMIT=unknown
ENV GIT_COMMIT=$GIT_COMMIT

# Build, bundle, and pack self-contained CLI tarball
RUN HUSKY=0 npm run build && \
    HUSKY=0 npm run bundle && \
    node scripts/prepare-npm-release.js && \
    mkdir -p /tmp/dist-npm && \
    npm pack ./dist/npm-release/m31a --pack-destination /tmp/dist-npm

# ---- Stage 2: Runtime ----
FROM docker.io/library/node:22-slim

ARG SANDBOX_NAME="m31a-sandbox"
ARG CLI_VERSION_ARG
ENV SANDBOX="$SANDBOX_NAME"
ENV CLI_VERSION=$CLI_VERSION_ARG

# install minimal set of packages and update global npm toolchain, then clean up
RUN apt-get update && apt-get dist-upgrade -y && apt-get install -y --no-install-recommends \
  python3 \
  make \
  g++ \
  man-db \
  curl \
  dnsutils \
  less \
  jq \
  bc \
  gh \
  git \
  unzip \
  rsync \
  ripgrep \
  procps \
  psmisc \
  lsof \
  socat \
  ca-certificates \
  && npm install -g npm@^11 --no-audit --no-fund \
  && mkdir -p /tmp/npm-deps \
  && npm install --prefix /tmp/npm-deps glob@14.0.3 minimatch@11.0.2 --no-save --no-audit --no-fund \
  && rm -rf /usr/local/lib/node_modules/npm/node_modules/glob /usr/local/lib/node_modules/npm/node_modules/minimatch \
  && cp -r /tmp/npm-deps/node_modules/glob /usr/local/lib/node_modules/npm/node_modules/glob \
  && cp -r /tmp/npm-deps/node_modules/minimatch /usr/local/lib/node_modules/npm/node_modules/minimatch \
  && for extra in /tmp/npm-deps/node_modules/* /tmp/npm-deps/node_modules/@isaacs/*; do \
       [ -e "$extra" ] || continue; \
       rel="${extra#/tmp/npm-deps/node_modules/}"; \
       if [ ! -e "/usr/local/lib/node_modules/npm/node_modules/$rel" ]; then \
         mkdir -p "$(dirname "/usr/local/lib/node_modules/npm/node_modules/$rel")"; \
         cp -r "$extra" "/usr/local/lib/node_modules/npm/node_modules/$rel"; \
       fi; \
     done \
  && rm -rf /tmp/npm-deps /root/.npm \
  && npm cache clean --force \
  && apt-get clean \
  && rm -rf /var/lib/apt/lists/*

# set up npm global package folder under /usr/local/share
# give it to non-root user node, already set up in base image
RUN mkdir -p /usr/local/share/npm-global/bin /usr/local/share/npm-global/lib \
  && chown -R node:node /usr/local/share/npm-global
ENV NPM_CONFIG_PREFIX=/usr/local/share/npm-global
ENV PATH=$PATH:/usr/local/share/npm-global/bin

# create container-internal .m31a directory owned by non-root node user
RUN mkdir -p /home/node/.m31a \
  && chown -R node:node /home/node/.m31a

# switch to non-root user node
USER node

# install m31a globally from the built self-contained tarball and clean up
COPY --from=builder --chown=node:node /tmp/dist-npm/m31a-*.tgz /tmp/m31a.tgz
RUN npm install -g /tmp/m31a.tgz --omit=dev --no-audit --no-fund \
  && rm -f /tmp/m31a.tgz \
  && m31a --version > /dev/null \
  && npm cache clean --force \
  && rm -rf /home/node/.npm

# default entrypoint when none specified
ENTRYPOINT ["/usr/local/share/npm-global/bin/m31a"]
