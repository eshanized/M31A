/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import * as fs from 'node:fs';
import * as path from 'node:path';
import { performInit } from '@m31a/core';
import type {
  Command,
  CommandContext,
  CommandExecutionResponse,
} from './types.js';

export class InitCommand implements Command {
  name = 'init';
  description = 'Analyzes the project and creates a tailored M31A.md file';
  requiresWorkspace = true;

  async execute(
    context: CommandContext,
    _args: string[] = [],
  ): Promise<CommandExecutionResponse> {
    const targetDir = context.agentContext.config.getTargetDir();
    if (!targetDir) {
      throw new Error('Command requires a workspace.');
    }

    const geminiMdPath = path.join(targetDir, 'M31A.md');
    const result = performInit(fs.existsSync(geminiMdPath));

    switch (result.type) {
      case 'message':
        return {
          name: this.name,
          data: result,
        };
      case 'submit_prompt':
        fs.writeFileSync(geminiMdPath, '', 'utf8');

        if (typeof result.content !== 'string') {
          throw new Error('Init command content must be a string.');
        }

        // Inform the user since we can't trigger the UI-based interactive agent loop here directly.
        // We output the prompt text they can use to re-trigger the generation manually,
        // or just seed the M31A.md file as we've done above.
        return {
          name: this.name,
          data: {
            type: 'message',
            messageType: 'info',
            content: `A template M31A.md has been created at ${geminiMdPath}.\n\nTo populate it with project context, you can run the following prompt in a new chat:\n\n${result.content}`,
          },
        };

      default:
        throw new Error('Unknown result type from performInit');
    }
  }
}
