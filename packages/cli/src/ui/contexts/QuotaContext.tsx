/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { createContext, useContext } from 'react';
import type { QuotaStats } from '../types.js';
import type { UserTierId } from '@m31a/core';

export interface QuotaState {
  userTier?: UserTierId;
  stats?: QuotaStats;
}

export const QuotaContext = createContext<QuotaState | null>(null);

export const useQuotaState = () => {
  const context = useContext(QuotaContext);
  if (!context) {
    throw new Error('useQuotaState must be used within a QuotaProvider');
  }
  return context;
};
