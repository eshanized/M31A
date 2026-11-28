/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import { MCPServerConfig, type Config } from './config.js';
import type {
  McpConfigDefinition,
  RequiredMcpServerConfig,
} from './tierTypes.js';

function isMcpConfigDefinition(
  value: McpConfigDefinition | Record<string, MCPServerConfig>,
): value is McpConfigDefinition {
  if ('requiredMcpServers' in value) {
    return true;
  }
  const entry = Object.entries(value).find(([k]) => k === 'mcpServers');
  if (!entry) {
    return false;
  }
  return entry[1] === undefined || !(entry[1] instanceof MCPServerConfig);
}

export function getAdminErrorMessage(
  featureName: string,
  _config: Config | undefined,
): string {
  return `${featureName} is disabled by your administrator.`;
}

export function getAdminBlockedMcpServersMessage(
  blockedServerNames: string[],
  _config: Config | undefined,
): string {
  return `The following MCP servers are blocked by your administrator: ${blockedServerNames.join(', ')}`;
}

export function applyAdminAllowlist(
  mcpServers: Record<string, MCPServerConfig> | undefined,
  adminAllowlist:
    | McpConfigDefinition
    | Record<string, MCPServerConfig>
    | undefined,
): {
  mcpServers: Record<string, MCPServerConfig>;
  blockedServerNames: string[];
} {
  if (!mcpServers) {
    return { mcpServers: {}, blockedServerNames: [] };
  }
  const allowlistServers: Record<string, MCPServerConfig> | undefined =
    adminAllowlist && isMcpConfigDefinition(adminAllowlist)
      ? adminAllowlist.mcpServers
      : adminAllowlist;

  if (!allowlistServers || Object.keys(allowlistServers).length === 0) {
    return { mcpServers, blockedServerNames: [] };
  }
  const filtered: Record<string, MCPServerConfig> = {};
  const blockedServerNames: string[] = [];
  for (const [name, localCfg] of Object.entries(mcpServers)) {
    if (Object.prototype.hasOwnProperty.call(allowlistServers, name)) {
      const adminCfg = allowlistServers[name];
      const {
        command: _c,
        args: _a,
        env: _e,
        cwd: _w,
        httpUrl: _h,
        tcp: _t,
        ...localRest
      } = localCfg;
      const { ...adminRest } = adminCfg;
      filtered[name] = {
        ...localRest,
        ...adminRest,
        trust: adminCfg.trust ?? localCfg.trust,
        timeout: adminCfg.timeout ?? localCfg.timeout,
        includeTools: adminCfg.includeTools ?? localCfg.includeTools,
        excludeTools: adminCfg.excludeTools ?? localCfg.excludeTools,
      };
    } else {
      blockedServerNames.push(name);
    }
  }
  return { mcpServers: filtered, blockedServerNames };
}

export function applyRequiredServers(
  mcpServers: Record<string, MCPServerConfig>,
  requiredConfig: Record<string, RequiredMcpServerConfig>,
): {
  mcpServers: Record<string, MCPServerConfig>;
  requiredServerNames: string[];
} {
  const merged: Record<string, MCPServerConfig> = { ...mcpServers };
  const requiredServerNames: string[] = [];
  for (const [name, reqCfg] of Object.entries(requiredConfig)) {
    merged[name] = {
      url: reqCfg.url,
      type: reqCfg.type,
      authProviderType: reqCfg.authProviderType,
      oauth: reqCfg.oauth,
      targetAudience: reqCfg.targetAudience,
      targetServiceAccount: reqCfg.targetServiceAccount,
      headers: reqCfg.headers,
      trust: reqCfg.trust,
      timeout: reqCfg.timeout,
      description: reqCfg.description,
      includeTools: reqCfg.includeTools,
      excludeTools: reqCfg.excludeTools,
    };
    requiredServerNames.push(name);
  }
  return { mcpServers: merged, requiredServerNames };
}
