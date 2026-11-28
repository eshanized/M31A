/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

export interface ModelResolutionContext {
  hasAccessToProModel?: boolean;
  requestedModel?: string;
  releaseChannel?: string;
}

/**
 * Interface for the ModelConfigService to break circular dependencies.
 */
export interface IModelConfigService {
  getModelDefinition(modelId: string):
    | {
        tier?: string;
        family?: string;
        isPreview?: boolean;
        displayName?: string;
        features?: {
          thinking?: boolean;
          multimodalToolUse?: boolean;
        };
      }
    | undefined;

  resolveModelId(
    requestedModel: string,
    context?: ModelResolutionContext,
  ): string;

  resolveClassifierModelId(
    tier: string,
    requestedModel: string,
    context?: ModelResolutionContext,
  ): string;
}

/**
 * Interface defining the minimal configuration required for model capability checks.
 * This helps break circular dependencies between Config and models.ts.
 */
export interface ModelCapabilityContext {
  readonly modelConfigService: IModelConfigService;
  getExperimentalDynamicModelConfiguration(): boolean;
}

export const DEFAULT_M31A_MODEL = 'nvidia/nemotron-3-ultra-550b-a55b';
export const DEFAULT_M31A_EMBEDDING_MODEL = 'nvidia/nv-embed-v1';

export const DEFAULT_M31A_FLASH_MODEL = DEFAULT_M31A_MODEL;
export const DEFAULT_M31A_FLASH_LITE_MODEL = DEFAULT_M31A_MODEL;

export const VALID_M31A_MODELS = new Set([DEFAULT_M31A_MODEL]);

/** @deprecated Use DEFAULT_M31A_MODEL or M31A_MODEL_ALIAS_AUTO instead. */
export const DEFAULT_M31A_MODEL_AUTO = 'auto';

// Model aliases for user convenience.
export const M31A_MODEL_ALIAS_AUTO = 'auto';
export const M31A_MODEL_ALIAS_PRO = 'pro';
export const M31A_MODEL_ALIAS_FLASH = 'flash';
export const M31A_MODEL_ALIAS_FLASH_LITE = 'flash-lite';

// Cap the thinking at 8192 to prevent run-away thinking loops.
export const DEFAULT_THINKING_MODE = 8192;

export function getAutoModelDescription() {
  return `Let M31A use the default model: ${DEFAULT_M31A_MODEL}`;
}

/**
 * Resolves the requested model alias (e.g., 'auto', 'pro', 'flash', 'flash-lite')
 * to a concrete model name.
 *
 * @param requestedModel The model alias or concrete model name requested by the user.
 * @param config Optional config object for dynamic model configuration.
 * @returns The resolved concrete model name.
 */
export function resolveModel(
  requestedModel: string,
  config?: ModelCapabilityContext,
): string {
  // Defensive check against non-string inputs at runtime
  const normalizedModel = Array.isArray(requestedModel)
    ? String(requestedModel.at(-1) ?? '').trim() || ''
    : typeof requestedModel !== 'string'
      ? String(requestedModel ?? '').trim() || ''
      : requestedModel.trim() || '';

  if (config?.getExperimentalDynamicModelConfiguration?.() === true) {
    return config.modelConfigService.resolveModelId(normalizedModel, {});
  }

  switch (normalizedModel) {
    case M31A_MODEL_ALIAS_AUTO:
    case M31A_MODEL_ALIAS_PRO:
    case M31A_MODEL_ALIAS_FLASH:
    case M31A_MODEL_ALIAS_FLASH_LITE:
    case '': {
      return DEFAULT_M31A_MODEL;
    }
    default: {
      return normalizedModel;
    }
  }
}

/**
 * Resolves the appropriate model based on the classifier's decision.
 *
 * @param requestedModel The current requested model (e.g. auto).
 * @param modelAlias The alias selected by the classifier ('flash' or 'pro').
 * @param config Optional config object for dynamic model configuration.
 * @returns The resolved concrete model name.
 */
export function resolveClassifierModel(
  requestedModel: string,
  modelAlias: string,
  config?: ModelCapabilityContext,
): string {
  if (config?.getExperimentalDynamicModelConfiguration?.() === true) {
    return config.modelConfigService.resolveClassifierModelId(
      modelAlias,
      requestedModel,
      {},
    );
  }

  if (modelAlias === M31A_MODEL_ALIAS_FLASH) {
    return resolveModel(M31A_MODEL_ALIAS_FLASH, config);
  }

  if (modelAlias === M31A_MODEL_ALIAS_FLASH_LITE) {
    return resolveModel(M31A_MODEL_ALIAS_FLASH_LITE, config);
  }

  return resolveModel(requestedModel, config);
}

export function getDisplayString(
  model: string,
  config?: ModelCapabilityContext,
) {
  if (config?.getExperimentalDynamicModelConfiguration?.() === true) {
    const definition = config.modelConfigService.getModelDefinition(model);
    if (definition?.displayName) {
      return definition.displayName;
    }
  }

  switch (model) {
    case DEFAULT_M31A_MODEL:
    case M31A_MODEL_ALIAS_PRO:
    case M31A_MODEL_ALIAS_FLASH:
    case M31A_MODEL_ALIAS_FLASH_LITE:
      return DEFAULT_M31A_MODEL;
    case M31A_MODEL_ALIAS_AUTO:
      return 'Auto';
    default:
      return model;
  }
}

/**
 * Checks if the model is a preview model.
 *
 * @param model The model name to check.
 * @param config Optional config object for dynamic model configuration.
 * @returns True if the model is a preview model.
 */
export function isPreviewModel(
  model: string,
  config?: ModelCapabilityContext,
): boolean {
  if (!model || model === 'none' || model === DEFAULT_M31A_MODEL) {
    return false;
  }
  if (config?.getExperimentalDynamicModelConfiguration?.() === true) {
    return (
      config.modelConfigService.getModelDefinition(model)?.isPreview === true
    );
  }

  return model.includes('-preview');
}

/**
 * Checks if the model is a Pro model.
 *
 * @param model The model name to check.
 * @param config Optional config object for dynamic model configuration.
 * @returns True if the model is a Pro model.
 */
export function isProModel(
  model: string,
  config?: ModelCapabilityContext,
): boolean {
  if (config?.getExperimentalDynamicModelConfiguration?.() === true) {
    return config.modelConfigService.getModelDefinition(model)?.tier === 'pro';
  }
  return model.toLowerCase().includes('pro');
}

/**
 * Checks if the model is a Gemini 3 model.
 *
 * @param model The model name to check.
 * @param config Optional config object for dynamic model configuration.
 * @returns True if the model is a Gemini 3 model.
 */
export function isGemini3Model(
  model: string,
  config?: ModelCapabilityContext,
): boolean {
  if (config?.getExperimentalDynamicModelConfiguration?.() === true) {
    // Legacy behavior resolves the model first.
    const resolved = resolveModel(model, config);
    const definition = config.modelConfigService.getModelDefinition(resolved);
    if (definition?.family) {
      return definition.family === 'gemini-3';
    }
    return /^gemini-3(\.|-|$)/.test(resolved);
  }

  const resolved = resolveModel(model);
  return /^gemini-3(\.|-|$)/.test(resolved);
}

/**
 * Checks if the model is a Gemini 2.x model.
 *
 * @param model The model name to check.
 * @param config Optional config object for dynamic model configuration.
 * @returns True if the model is a Gemini-2.x model.
 */
export function isGemini2Model(
  model: string,
  config?: ModelCapabilityContext,
): boolean {
  if (config?.getExperimentalDynamicModelConfiguration?.() === true) {
    const resolved = resolveModel(model, config);
    const definition = config.modelConfigService.getModelDefinition(resolved);
    if (definition?.family) {
      return (
        definition.family === 'gemini-2' || definition.family === 'gemini-2.5'
      );
    }
    return /^gemini-2(\.|-|$)/.test(resolved);
  }

  // This is legacy behavior, will remove this when gemini 2 models are no
  // longer needed.
  const resolved = resolveModel(model);
  return /^gemini-2(\.|-|$)/.test(resolved);
}

/**
 * Checks if the model is an auto model.
 *
 * @param model The model name to check.
 * @param config Optional config object for dynamic model configuration.
 * @returns True if the model is an auto model.
 */
export function isAutoModel(
  model: string,
  config?: ModelCapabilityContext,
): boolean {
  if (config?.getExperimentalDynamicModelConfiguration?.() === true) {
    return config.modelConfigService.getModelDefinition(model)?.tier === 'auto';
  }
  return model === M31A_MODEL_ALIAS_AUTO || model === DEFAULT_M31A_MODEL_AUTO;
}

/**
 * Checks if the model supports multimodal function responses (multimodal data nested within function response).
 *
 * @param model The model name to check.
 * @returns True if the model supports multimodal function responses.
 */
export function supportsMultimodalFunctionResponse(
  model: string,
  config?: ModelCapabilityContext,
): boolean {
  const resolved = resolveModel(model, config);
  if (config?.getExperimentalDynamicModelConfiguration?.() === true) {
    const definition = config.modelConfigService.getModelDefinition(resolved);
    if (definition?.features?.multimodalToolUse !== undefined) {
      return definition.features.multimodalToolUse;
    }
  }
  return /^gemini-3(\.|-|$)/.test(resolved);
}

/**
 * Checks if the given model is considered active based on the current configuration.
 *
 * @param model The model name to check.
 * @returns True if the model is active.
 */
export function isActiveModel(model: string): boolean {
  return VALID_M31A_MODELS.has(model) && model !== 'none';
}
