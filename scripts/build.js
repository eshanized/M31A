/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

import { execSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, '..');

// npm install if node_modules was removed (e.g. via npm run clean or scripts/clean.js)
if (!existsSync(join(root, 'node_modules'))) {
  execSync('npm install', { stdio: 'inherit', cwd: root });
}

// build all workspaces/packages
execSync('npm run generate', { stdio: 'inherit', cwd: root });

// Build @m31a/genai first because @m31a/core and @m31a/cli depend on it,
// then @m31a/core because the remaining workspaces depend on it.
console.log('Building @m31a/genai...');
execSync('npm run build -w @m31a/genai', {
  stdio: 'inherit',
  cwd: root,
});

console.log('Building @m31a/core...');
execSync('npm run build -w @m31a/core', {
  stdio: 'inherit',
  cwd: root,
});

const workspaceInfo = JSON.parse(
  execSync('npm query .workspace --json', { cwd: root, encoding: 'utf-8' }),
);
const remainingWorkspaces = workspaceInfo
  .map((w) => w.name)
  .filter((name) => name !== '@m31a/genai' && name !== '@m31a/core');

if (process.env.CI) {
  console.log(
    'CI environment detected. Building remaining workspaces sequentially...',
  );
  for (const ws of remainingWorkspaces) {
    execSync(`npm run build -w ${ws}`, { stdio: 'inherit', cwd: root });
  }
} else {
  console.log('Building other workspaces in parallel...');
  execSync(
    `npx --no-install npm-run-all --parallel ${remainingWorkspaces.map((w) => `"build -w ${w}"`).join(' ')}`,
    { stdio: 'inherit', cwd: root },
  );
}

// also build container image if sandboxing is enabled
// skip (-s) npm install + build since we did that above
try {
  execSync('node scripts/sandbox_command.js -q', {
    stdio: 'inherit',
    cwd: root,
  });
  if (
    process.env.BUILD_SANDBOX === '1' ||
    process.env.BUILD_SANDBOX === 'true'
  ) {
    execSync('node scripts/build_sandbox.js -s', {
      stdio: 'inherit',
      cwd: root,
    });
  }
} catch {
  // ignore
}
