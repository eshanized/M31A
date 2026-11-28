/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import fs from 'node:fs';
import path from 'node:path';

const rootDir = process.cwd();
const sourceBundleDir = path.resolve(rootDir, 'bundle');
const releaseDir = path.resolve(rootDir, 'dist/npm-release/m31a');

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(path.resolve(rootDir, filePath), 'utf-8'));
}

if (!fs.existsSync(sourceBundleDir)) {
  console.error(
    'Error: bundle/ directory not found at project root. Please run `npm run bundle` first.',
  );
  process.exit(1);
}

fs.rmSync(releaseDir, { recursive: true, force: true });
fs.mkdirSync(releaseDir, { recursive: true });

fs.cpSync(sourceBundleDir, path.join(releaseDir, 'bundle'), {
  recursive: true,
});
fs.rmSync(path.join(releaseDir, 'bundle/esbuild.json'), { force: true });

for (const file of ['README.md', 'LICENSE']) {
  const src = path.resolve(rootDir, file);
  if (fs.existsSync(src)) {
    fs.copyFileSync(src, path.join(releaseDir, file));
  }
}

const rootPkg = readJson('package.json');
const optionalDependencies = { ...(rootPkg.optionalDependencies || {}) };
delete optionalDependencies['m31a-devtools'];

const publishPkg = {
  name: rootPkg.name || 'm31a',
  version: rootPkg.version,
  description:
    'M31A — open-source agentic coding terminal powered by NVIDIA NIM',
  keywords: ['ai', 'cli', 'coding-agent', 'nvidia-nim', 'm31a', 'terminal'],
  author: rootPkg.author,
  license: rootPkg.license,
  engines: rootPkg.engines,
  type: rootPkg.type,
  repository: rootPkg.repository,
  homepage: rootPkg.homepage,
  bugs: rootPkg.bugs,
  bin: {
    m31a: 'bundle/m31a.js',
  },
  files: ['bundle/', 'README.md', 'LICENSE'],
  optionalDependencies,
};

fs.writeFileSync(
  path.join(releaseDir, 'package.json'),
  JSON.stringify(publishPkg, null, 2) + '\n',
);

console.log(
  `Prepared unscoped npm package "${publishPkg.name}@${publishPkg.version}" in ${releaseDir}`,
);
