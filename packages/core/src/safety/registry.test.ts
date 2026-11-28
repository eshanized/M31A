/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { CheckerRegistry } from './registry.js';
import { InProcessCheckerType } from '../policy/types.js';
import { AllowedPathChecker } from './built-in.js';
import { ConsecaSafetyChecker } from './conseca/conseca.js';

describe('CheckerRegistry', () => {
  let registry: CheckerRegistry;
  beforeEach(() => {
    registry = new CheckerRegistry();
  });

  it('should resolve built-in in-process checkers', () => {
    const allowedPathChecker = registry.resolveInProcess(
      InProcessCheckerType.ALLOWED_PATH,
    );
    expect(allowedPathChecker).toBeInstanceOf(AllowedPathChecker);

    const consecaChecker = registry.resolveInProcess(
      InProcessCheckerType.CONSECA,
    );
    expect(consecaChecker).toBeInstanceOf(ConsecaSafetyChecker);
  });

  it('should throw for unknown in-process checkers', () => {
    expect(() => registry.resolveInProcess('unknown-checker')).toThrow(
      'Unknown in-process checker "unknown-checker"',
    );
  });

  it('should validate checker names', () => {
    expect(() => registry.resolveInProcess('invalid name!')).toThrow(
      'Invalid checker name',
    );
    expect(() => registry.resolveInProcess('../escape')).toThrow(
      'Invalid checker name',
    );
  });
});
