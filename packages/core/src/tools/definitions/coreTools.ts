/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

/**
 * Orchestrator for tool definitions.
 */

import type { ToolDefinition } from './types.js';
import { DEFAULT_TOOL_SET } from './model-family-sets/default.js';
import {
  getShellDeclaration,
  getExitPlanModeDeclaration,
  getActivateSkillDeclaration,
  getUpdateTopicDeclaration,
} from './dynamic-declaration-helpers.js';

// Re-export names for compatibility
export {
  GLOB_TOOL_NAME,
  GREP_TOOL_NAME,
  LS_TOOL_NAME,
  READ_FILE_TOOL_NAME,
  SHELL_TOOL_NAME,
  WRITE_FILE_TOOL_NAME,
  EDIT_TOOL_NAME,
  APPLY_PATCH_TOOL_NAME,
  APPLY_PATCH_PARAM_PATCH_TEXT,
  SESSION_SEARCH_TOOL_NAME,
  SESSION_SEARCH_PARAM_LIMIT,
  EXECUTE_TOOL_NAME,
  EXECUTE_PARAM_CODE,
  WEB_SEARCH_TOOL_NAME,
  WRITE_TODOS_TOOL_NAME,
  WEB_FETCH_TOOL_NAME,
  READ_MANY_FILES_TOOL_NAME,
  GET_INTERNAL_DOCS_TOOL_NAME,
  ACTIVATE_SKILL_TOOL_NAME,
  ASK_USER_TOOL_NAME,
  EXIT_PLAN_MODE_TOOL_NAME,
  ENTER_PLAN_MODE_TOOL_NAME,
  UPDATE_TOPIC_TOOL_NAME,
  UPDATE_TOPIC_DISPLAY_NAME,
  COMPLETE_TASK_TOOL_NAME,
  COMPLETE_TASK_DISPLAY_NAME,
  READ_MCP_RESOURCE_TOOL_NAME,
  LIST_MCP_RESOURCES_TOOL_NAME,
  // Shared parameter names
  PARAM_FILE_PATH,
  PARAM_DIR_PATH,
  PARAM_PATTERN,
  PARAM_CASE_SENSITIVE,
  PARAM_RESPECT_GIT_IGNORE,
  PARAM_RESPECT_M31A_IGNORE,
  PARAM_FILE_FILTERING_OPTIONS,
  PARAM_DESCRIPTION,
  // Tool-specific parameter names
  READ_FILE_PARAM_START_LINE,
  READ_FILE_PARAM_END_LINE,
  WRITE_FILE_PARAM_CONTENT,
  GREP_PARAM_INCLUDE_PATTERN,
  GREP_PARAM_EXCLUDE_PATTERN,
  GREP_PARAM_NAMES_ONLY,
  GREP_PARAM_MAX_MATCHES_PER_FILE,
  GREP_PARAM_TOTAL_MAX_MATCHES,
  GREP_PARAM_FIXED_STRINGS,
  GREP_PARAM_CONTEXT,
  GREP_PARAM_AFTER,
  GREP_PARAM_BEFORE,
  GREP_PARAM_NO_IGNORE,
  EDIT_PARAM_INSTRUCTION,
  EDIT_PARAM_OLD_STRING,
  EDIT_PARAM_NEW_STRING,
  EDIT_PARAM_ALLOW_MULTIPLE,
  LS_PARAM_IGNORE,
  SHELL_PARAM_COMMAND,
  SHELL_PARAM_IS_BACKGROUND,
  WEB_SEARCH_PARAM_QUERY,
  WEB_FETCH_PARAM_PROMPT,
  READ_MANY_PARAM_INCLUDE,
  READ_MANY_PARAM_EXCLUDE,
  READ_MANY_PARAM_RECURSIVE,
  READ_MANY_PARAM_USE_DEFAULT_EXCLUDES,
  TODOS_PARAM_TODOS,
  TODOS_ITEM_PARAM_DESCRIPTION,
  TODOS_ITEM_PARAM_STATUS,
  DOCS_PARAM_PATH,
  ASK_USER_PARAM_QUESTIONS,
  ASK_USER_QUESTION_PARAM_QUESTION,
  ASK_USER_QUESTION_PARAM_HEADER,
  ASK_USER_QUESTION_PARAM_TYPE,
  ASK_USER_QUESTION_PARAM_OPTIONS,
  ASK_USER_QUESTION_PARAM_MULTI_SELECT,
  ASK_USER_QUESTION_PARAM_PLACEHOLDER,
  ASK_USER_OPTION_PARAM_LABEL,
  ASK_USER_OPTION_PARAM_DESCRIPTION,
  PLAN_MODE_PARAM_REASON,
  EXIT_PLAN_PARAM_PLAN_FILENAME,
  SKILL_PARAM_NAME,
  TOPIC_PARAM_TITLE,
  TOPIC_PARAM_SUMMARY,
  TOPIC_PARAM_STRATEGIC_INTENT,
} from './base-declarations.js';

// Re-export default tool set
export { DEFAULT_TOOL_SET } from './model-family-sets/default.js';

export const READ_FILE_DEFINITION: ToolDefinition = {
  get base() {
    return DEFAULT_TOOL_SET.read_file;
  },
};

export const WRITE_FILE_DEFINITION: ToolDefinition = {
  get base() {
    return DEFAULT_TOOL_SET.write_file;
  },
};

export const GREP_DEFINITION: ToolDefinition = {
  get base() {
    return DEFAULT_TOOL_SET.grep_search;
  },
};

export const RIP_GREP_DEFINITION: ToolDefinition = {
  get base() {
    return DEFAULT_TOOL_SET.grep_search_ripgrep;
  },
};

export const WEB_SEARCH_DEFINITION: ToolDefinition = {
  get base() {
    return DEFAULT_TOOL_SET.google_web_search;
  },
};

export const EDIT_DEFINITION: ToolDefinition = {
  get base() {
    return DEFAULT_TOOL_SET.replace;
  },
};

export const APPLY_PATCH_DEFINITION: ToolDefinition = {
  get base() {
    return DEFAULT_TOOL_SET.apply_patch;
  },
};

export const SESSION_SEARCH_DEFINITION: ToolDefinition = {
  get base() {
    return DEFAULT_TOOL_SET.session_search;
  },
};

export const EXECUTE_DEFINITION: ToolDefinition = {
  get base() {
    return DEFAULT_TOOL_SET.execute;
  },
};

export const GLOB_DEFINITION: ToolDefinition = {
  get base() {
    return DEFAULT_TOOL_SET.glob;
  },
};

export const LS_DEFINITION: ToolDefinition = {
  get base() {
    return DEFAULT_TOOL_SET.list_directory;
  },
};

export const WEB_FETCH_DEFINITION: ToolDefinition = {
  get base() {
    return DEFAULT_TOOL_SET.web_fetch;
  },
};

export const READ_MANY_FILES_DEFINITION: ToolDefinition = {
  get base() {
    return DEFAULT_TOOL_SET.read_many_files;
  },
};

export const WRITE_TODOS_DEFINITION: ToolDefinition = {
  get base() {
    return DEFAULT_TOOL_SET.write_todos;
  },
};

export const GET_INTERNAL_DOCS_DEFINITION: ToolDefinition = {
  get base() {
    return DEFAULT_TOOL_SET.get_internal_docs;
  },
};

export const ASK_USER_DEFINITION: ToolDefinition = {
  get base() {
    return DEFAULT_TOOL_SET.ask_user;
  },
};

export const ENTER_PLAN_MODE_DEFINITION: ToolDefinition = {
  get base() {
    return DEFAULT_TOOL_SET.enter_plan_mode;
  },
};

export const UPDATE_TOPIC_DEFINITION: ToolDefinition = {
  get base() {
    return getUpdateTopicDeclaration();
  },
};

export {
  getShellToolDescription,
  getCommandDescription,
} from './dynamic-declaration-helpers.js';

export function getShellDefinition(
  enableInteractiveShell: boolean,
  enableEfficiency: boolean,
  enableToolSandboxing: boolean = false,
): ToolDefinition {
  return {
    base: getShellDeclaration(
      enableInteractiveShell,
      enableEfficiency,
      enableToolSandboxing,
    ),
  };
}

export function getExitPlanModeDefinition(): ToolDefinition {
  return {
    base: getExitPlanModeDeclaration(),
  };
}

export function getActivateSkillDefinition(
  skillNames: string[],
): ToolDefinition {
  return {
    base: getActivateSkillDeclaration(skillNames),
  };
}

export const READ_MCP_RESOURCE_DEFINITION: ToolDefinition = {
  get base() {
    return DEFAULT_TOOL_SET.read_mcp_resource;
  },
};

export const LIST_MCP_RESOURCES_DEFINITION: ToolDefinition = {
  get base() {
    return DEFAULT_TOOL_SET.list_mcp_resources;
  },
};
