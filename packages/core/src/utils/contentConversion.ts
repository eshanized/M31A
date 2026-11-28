/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

import type {
  Content,
  ContentListUnion,
  ContentUnion,
  Part,
  PartUnion,
} from '@m31a/genai';

export function toContents(contents: ContentListUnion): Content[] {
  if (Array.isArray(contents)) {
    return contents.map(toContent);
  }
  return [toContent(contents)];
}

function isPart(c: ContentUnion): c is PartUnion {
  return (
    typeof c === 'object' &&
    c !== null &&
    !Array.isArray(c) &&
    !('parts' in c) &&
    !('role' in c)
  );
}

function toContent(content: ContentUnion): Content {
  if (Array.isArray(content)) {
    return {
      role: 'user',
      parts: toParts(content),
    };
  }
  if (typeof content === 'string') {
    return {
      role: 'user',
      parts: [{ text: content }],
    };
  }
  if (!isPart(content)) {
    return {
      ...content,
      parts: content.parts
        ? toParts(content.parts.filter((p) => p != null))
        : [],
    };
  }
  return {
    role: 'user',
    parts: [toPart(content)],
  };
}

export function toParts(parts: PartUnion[]): Part[] {
  return parts.map(toPart);
}

function toPart(part: PartUnion): Part {
  if (typeof part === 'string') {
    return { text: part };
  }

  if ('thought' in part && part.thought) {
    const thoughtText = `[Thought: ${String(part.thought)}]`;

    const newPart = { ...part };
    delete (newPart as Record<string, unknown>)['thought'];

    const hasApiContent =
      'functionCall' in newPart ||
      'functionResponse' in newPart ||
      'inlineData' in newPart ||
      'fileData' in newPart;

    if (hasApiContent) {
      return newPart;
    }

    const existingText =
      'text' in newPart && typeof newPart.text === 'string' ? newPart.text : '';
    const combinedText = existingText
      ? `${existingText}\n${thoughtText}`
      : thoughtText;

    return {
      ...newPart,
      text: combinedText,
    };
  }

  return part;
}
