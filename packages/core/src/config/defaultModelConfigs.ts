/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import type { ModelConfigServiceConfig } from '../services/modelConfigService.js';
import { DEFAULT_THINKING_MODE, DEFAULT_M31A_MODEL } from './models.js';

// The default model configs. We use `base` as the parent for all of our model
// configs, while `chat-base`, a child of `base`, is the parent of the models
// we use in the interactive chat experience.
export const DEFAULT_MODEL_CONFIGS: ModelConfigServiceConfig = {
  aliases: {
    base: {
      modelConfig: {
        generateContentConfig: {
          temperature: 0,
          topP: 1,
        },
      },
    },
    'chat-base': {
      extends: 'base',
      modelConfig: {
        generateContentConfig: {
          thinkingConfig: {
            includeThoughts: true,
            thinkingBudget: DEFAULT_THINKING_MODE,
          },
          temperature: 1,
          topP: 0.95,
          topK: 64,
        },
      },
    },
    'flash-base': {
      extends: 'base',
      modelConfig: {
        model: DEFAULT_M31A_MODEL,
      },
    },
    classifier: {
      extends: 'base',
      modelConfig: {
        model: 'flash-lite',
        generateContentConfig: {
          maxOutputTokens: 1024,
          thinkingConfig: {
            thinkingBudget: 512,
          },
        },
      },
    },
    'prompt-completion': {
      extends: 'base',
      modelConfig: {
        model: 'flash-lite',
        generateContentConfig: {
          temperature: 0.3,
          maxOutputTokens: 16000,
          thinkingConfig: {
            thinkingBudget: 0,
          },
        },
      },
    },
    'fast-ack-helper': {
      extends: 'base',
      modelConfig: {
        model: 'flash-lite',
        generateContentConfig: {
          temperature: 0.2,
          maxOutputTokens: 120,
          thinkingConfig: {
            thinkingBudget: 0,
          },
        },
      },
    },
    'edit-corrector': {
      extends: 'base',
      modelConfig: {
        model: 'flash-lite',
        generateContentConfig: {
          thinkingConfig: {
            thinkingBudget: 0,
          },
        },
      },
    },
    'summarizer-default': {
      extends: 'base',
      modelConfig: {
        model: 'flash-lite',
        generateContentConfig: {
          maxOutputTokens: 2000,
        },
      },
    },
    'summarizer-shell': {
      extends: 'base',
      modelConfig: {
        model: 'flash-lite',
        generateContentConfig: {
          maxOutputTokens: 2000,
        },
      },
    },
    'web-search': {
      extends: 'flash-base',
      modelConfig: {
        generateContentConfig: {
          tools: [{ googleSearch: {} }],
        },
      },
    },
    'web-fetch': {
      extends: 'flash-base',
      modelConfig: {
        generateContentConfig: {
          tools: [{ urlContext: {} }],
        },
      },
    },
    'web-fetch-fallback': {
      extends: 'flash-base',
      modelConfig: {},
    },
    'loop-detection': {
      extends: 'flash-base',
      modelConfig: {},
    },
    'loop-detection-double-check': {
      extends: 'base',
      modelConfig: {
        model: DEFAULT_M31A_MODEL,
      },
    },
    'llm-edit-fixer': {
      extends: 'flash-base',
      modelConfig: {},
    },
    'next-speaker-checker': {
      extends: 'flash-base',
      modelConfig: {},
    },
    'context-snapshotter': {
      extends: 'flash-base',
      modelConfig: {
        generateContentConfig: {
          temperature: 1,
          topP: 0.95,
          topK: 64,
        },
      },
    },
    'chat-compression-default': {
      modelConfig: {
        model: DEFAULT_M31A_MODEL,
      },
    },
    'agent-history-provider-summarizer': {
      modelConfig: {
        model: DEFAULT_M31A_MODEL,
      },
    },
  },
  overrides: [
    {
      match: { model: 'chat-base', isRetry: true },
      modelConfig: {
        generateContentConfig: {
          temperature: 1,
        },
      },
    },
  ],
  modelDefinitions: {
    // Aliases
    auto: {
      displayName: 'Auto',
      tier: 'auto',
      isPreview: false,
      isVisible: true,
      features: { thinking: true, multimodalToolUse: false },
    },
    pro: {
      tier: 'pro',
      isPreview: false,
      isVisible: false,
      features: { thinking: true, multimodalToolUse: false },
    },
    flash: {
      tier: 'flash',
      isPreview: false,
      isVisible: false,
      features: { thinking: false, multimodalToolUse: false },
    },
    'flash-lite': {
      tier: 'flash-lite',
      isPreview: false,
      isVisible: false,
      features: { thinking: false, multimodalToolUse: false },
    },
    // Concrete Model
    [DEFAULT_M31A_MODEL]: {
      displayName: DEFAULT_M31A_MODEL,
      tier: 'pro',
      family: 'nemotron',
      isPreview: false,
      isVisible: true,
      features: { thinking: true, multimodalToolUse: false },
    },
  },
  modelIdResolutions: {
    [DEFAULT_M31A_MODEL]: {
      default: DEFAULT_M31A_MODEL,
    },
    auto: {
      default: DEFAULT_M31A_MODEL,
    },
    pro: {
      default: DEFAULT_M31A_MODEL,
    },
    flash: {
      default: DEFAULT_M31A_MODEL,
    },
    'flash-lite': {
      default: DEFAULT_M31A_MODEL,
    },
  },
  classifierIdResolutions: {
    flash: {
      default: DEFAULT_M31A_MODEL,
    },
    pro: {
      default: DEFAULT_M31A_MODEL,
    },
  },
  modelChains: {
    default: [
      {
        model: DEFAULT_M31A_MODEL,
        isLastResort: true,
        actions: {
          terminal: 'prompt',
          transient: 'prompt',
          not_found: 'prompt',
          unknown: 'prompt',
        },
        stateTransitions: {
          terminal: 'terminal',
          transient: 'terminal',
          not_found: 'terminal',
          unknown: 'terminal',
        },
      },
    ],
    'auto-default': [
      {
        model: DEFAULT_M31A_MODEL,
        isLastResort: true,
        maxAttempts: 3,
        actions: {
          terminal: 'prompt',
          transient: 'silent',
          not_found: 'prompt',
          unknown: 'prompt',
        },
        stateTransitions: {
          terminal: 'terminal',
          transient: 'sticky_retry',
          not_found: 'terminal',
          unknown: 'terminal',
        },
      },
    ],
    lite: [
      {
        model: DEFAULT_M31A_MODEL,
        isLastResort: true,
        actions: {
          terminal: 'silent',
          transient: 'silent',
          not_found: 'silent',
          unknown: 'silent',
        },
        stateTransitions: {
          terminal: 'terminal',
          transient: 'terminal',
          not_found: 'terminal',
          unknown: 'terminal',
        },
      },
    ],
  },
};
