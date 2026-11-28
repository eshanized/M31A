/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { z } from 'zod';
import { AuthProviderType } from './config.js';

export const UserTierId = {
  FREE: 'free-tier',
  LEGACY: 'legacy-tier',
  STANDARD: 'standard-tier',
} as const;

export type UserTierId = (typeof UserTierId)[keyof typeof UserTierId] | string;

export enum IneligibleTierReasonCode {
  DASHER_USER = 'DASHER_USER',
  INELIGIBLE_ACCOUNT = 'INELIGIBLE_ACCOUNT',
  NON_USER_ACCOUNT = 'NON_USER_ACCOUNT',
  RESTRICTED_AGE = 'RESTRICTED_AGE',
  RESTRICTED_NETWORK = 'RESTRICTED_NETWORK',
  UNKNOWN = 'UNKNOWN',
  UNKNOWN_LOCATION = 'UNKNOWN_LOCATION',
  UNSUPPORTED_LOCATION = 'UNSUPPORTED_LOCATION',
  VALIDATION_REQUIRED = 'VALIDATION_REQUIRED',
}

export interface IneligibleTier {
  reasonCode?: IneligibleTierReasonCode;
  reasonMessage?: string;
  tierId?: UserTierId;
  tierName?: string;
  validationErrorMessage?: string;
  validationUrl?: string;
  validationUrlLinkText?: string;
  validationLearnMoreUrl?: string;
  validationLearnMoreLinkText?: string;
}

export interface BucketInfo {
  remainingAmount?: string;
  remainingFraction?: number;
  resetTime?: string;
  tokenType?: string;
  modelId?: string;
}

export interface RetrieveUserQuotaResponse {
  buckets?: BucketInfo[];
}

const ExtensionsSettingSchema = z.object({
  extensionsEnabled: z.boolean().optional(),
});

const CliFeatureSettingSchema = z.object({
  extensionsSetting: ExtensionsSettingSchema.optional(),
  unmanagedCapabilitiesEnabled: z.boolean().optional(),
});

const McpServerConfigSchema = z.object({
  url: z.string().optional(),
  type: z.enum(['sse', 'http']).optional(),
  trust: z.boolean().optional(),
  includeTools: z.array(z.string()).optional(),
  excludeTools: z.array(z.string()).optional(),
});

const RequiredMcpServerOAuthSchema = z.object({
  scopes: z.array(z.string()).optional(),
  clientId: z.string().optional(),
  clientSecret: z.string().optional(),
});

export const RequiredMcpServerConfigSchema = z.object({
  url: z.string(),
  type: z.enum(['sse', 'http']),
  authProviderType: z.nativeEnum(AuthProviderType).optional(),
  oauth: RequiredMcpServerOAuthSchema.optional(),
  targetAudience: z.string().optional(),
  targetServiceAccount: z.string().optional(),
  headers: z.record(z.string()).optional(),
  trust: z.boolean().optional(),
  timeout: z.number().optional(),
  description: z.string().optional(),
  includeTools: z.array(z.string()).optional(),
  excludeTools: z.array(z.string()).optional(),
});

export type RequiredMcpServerConfig = z.infer<
  typeof RequiredMcpServerConfigSchema
>;

export const McpConfigDefinitionSchema = z.object({
  mcpServers: z.record(McpServerConfigSchema).optional(),
  requiredMcpServers: z.record(RequiredMcpServerConfigSchema).optional(),
});

export type McpConfigDefinition = z.infer<typeof McpConfigDefinitionSchema>;

const McpSettingSchema = z.object({
  mcpEnabled: z.boolean().optional(),
  mcpConfigJson: z.string().optional(),
});

export const AdminControlsSettingsSchema = z.object({
  strictModeDisabled: z.boolean().optional(),
  mcpSetting: z
    .object({
      mcpEnabled: z.boolean().optional(),
      mcpConfig: McpConfigDefinitionSchema.optional(),
      requiredMcpConfig: z.record(RequiredMcpServerConfigSchema).optional(),
    })
    .optional(),
  cliFeatureSetting: CliFeatureSettingSchema.optional(),
});

export type AdminControlsSettings = z.infer<typeof AdminControlsSettingsSchema>;

export const FetchAdminControlsResponseSchema = z.object({
  secureModeEnabled: z.boolean().optional(),
  strictModeDisabled: z.boolean().optional(),
  mcpSetting: McpSettingSchema.optional(),
  cliFeatureSetting: CliFeatureSettingSchema.optional(),
  adminControlsApplicable: z.boolean().optional(),
});

export type FetchAdminControlsResponse = z.infer<
  typeof FetchAdminControlsResponseSchema
>;
