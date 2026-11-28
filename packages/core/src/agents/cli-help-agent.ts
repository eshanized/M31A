/**
 * @license
 * Copyright 2025 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import type { AgentDefinition } from './types.js';
import { M31A_MODEL_ALIAS_FLASH } from '../config/models.js';
import { z } from 'zod';
import { GetInternalDocsTool } from '../tools/get-internal-docs.js';
import type { AgentLoopContext } from '../config/agent-loop-context.js';

const CliHelpReportSchema = z.object({
  answer: z
    .string()
    .describe('The detailed answer to the user question about M31A.'),
  sources: z
    .array(z.string())
    .describe('The documentation files used to answer the question.'),
});

/**
 * An agent specialized in answering questions about M31A itself,
 * using its own documentation and runtime state.
 */
export const CliHelpAgent = (
  context: AgentLoopContext,
): AgentDefinition<typeof CliHelpReportSchema> => ({
  name: 'cli_help',
  kind: 'local',
  displayName: 'CLI Help Agent',
  description:
    'Specialized agent for answering questions about the M31A application. Invoke this agent for questions regarding CLI features, configuration schemas (e.g., policies), or instructions on how to create custom subagents. It queries internal documentation to provide accurate usage guidance.',
  inputConfig: {
    inputSchema: {
      type: 'object',
      properties: {
        question: {
          type: 'string',
          description: 'The specific question about M31A.',
        },
      },
      required: ['question'],
    },
  },
  outputConfig: {
    outputName: 'report',
    description: 'The final answer and sources as a JSON object.',
    schema: CliHelpReportSchema,
  },

  processOutput: (output) => {
    if (!output) {
      return '';
    }
    const answer = output.answer?.trim() ?? '';
    const uniqueSources = Array.from(
      new Set(
        (output.sources ?? [])
          .map((s) => s?.trim())
          .filter((s) => s && s.length > 0),
      ),
    );

    if (uniqueSources.length > 0) {
      return `${answer}\n\n**Sources:**\n${uniqueSources.map((s) => `- ${s}`).join('\n')}`;
    }
    return answer;
  },

  modelConfig: {
    model: M31A_MODEL_ALIAS_FLASH,
    generateContentConfig: {
      temperature: 0.1,
      topP: 0.95,
      thinkingConfig: {
        includeThoughts: true,
        thinkingBudget: -1,
      },
    },
  },

  runConfig: {
    maxTimeMinutes: 3,
    maxTurns: 10,
  },

  toolConfig: {
    tools: [new GetInternalDocsTool(context.messageBus)],
  },

  promptConfig: {
    query:
      'Your task is to answer the following question about M31A:\n' +
      '<question>\n' +
      '${question}\n' +
      '</question>',
    systemPrompt:
      "You are **CLI Help Agent**, an expert on M31A. Your purpose is to provide accurate information about M31A's features, configuration, and current state.\n\n" +
      '### Runtime Context\n' +
      '- **CLI Version:** ${cliVersion}\n' +
      '- **Active Model:** ${activeModel}\n' +
      "- **Today's Date:** ${today}\n\n" +
      '### Instructions\n' +
      "1. **Explore Documentation**: Use the `get_internal_docs` tool to find answers. If you don't know where to start, call `get_internal_docs()` without arguments to see the full list of available documentation files.\n" +
      '2. **Be Precise**: Use the provided runtime context and documentation to give exact answers.\n' +
      '3. **Cite Sources**: Always include the specific documentation files you used in your final report.\n' +
      '4. **Non-Interactive**: You operate in a loop and cannot ask the user for more info. If the question is ambiguous, answer as best as you can with the information available.\n\n' +
      'You MUST call `complete_task` with a JSON report containing your `answer` and the `sources` you used.',
  },
});
