/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

export interface TailHeal {
  /** Characters to remove from the end of the right-trimmed text. */
  strip: number;
  /** Text to append after stripping. */
  append: string;
}

const RE_INLINE_DOLLAR_MATH =
  /\$\$(?!\$)((?:\\.|[^\\\n])*?(?:\\.|[^\\\n$]))\$\$|\$(?=[^\s$])((?:\\.|[^\\\n$])*?(?:\\\S|[^\s\\$]))\$(?!\w)/y;
const RE_INLINE_PAREN_MATH = /\\\(((?:\\.|[^\\\n])*?)\\\)/y;
const RE_LINK_TAIL = /(!?)\[([^\]\n]*)\]\(([^)\n]*)$/;
const RE_SETEXT_CANDIDATE = /^[ \t]*(?:-+|=+)[ \t]*$/;
const CODE_FENCE_LINE_RE = /^ *(`{3,}|~{3,})/;

function isSpace(c: number): boolean {
  return (
    c === 0x20 || c === 0x09 || c === 0x0a || c === 0x0d || Number.isNaN(c)
  );
}

function isAlnum(c: number): boolean {
  return (
    (c >= 0x30 && c <= 0x39) ||
    (c >= 0x41 && c <= 0x5a) ||
    (c >= 0x61 && c <= 0x7a)
  );
}

function rtrimLength(text: string): number {
  let end = text.length;
  while (end > 0 && isSpace(text.charCodeAt(end - 1))) end--;
  return end;
}

function compose(a: TailHeal, b: TailHeal): TailHeal {
  if (b.strip <= a.append.length) {
    return {
      strip: a.strip,
      append: a.append.slice(0, a.append.length - b.strip) + b.append,
    };
  }
  return {
    strip: a.strip + (b.strip - a.append.length),
    append: b.append,
  };
}

interface OpenDelimiter {
  ch: number;
  len: number;
  after: number;
  start: number;
}

/**
 * Computes the edit needed to heal incomplete inline markdown markers at the
 * end of a streaming inline run without mutating already-closed spans.
 */
export function computeTailHeal(input: string): TailHeal {
  const end = rtrimLength(input);
  let text = input.slice(0, end);
  if (!text) return { strip: 0, append: '' };

  let heal: TailHeal = { strip: 0, append: '' };

  // Unfinished inline link or image at the tail: `[label](https://...` renders as `label`.
  const lastOpen = text.lastIndexOf('[');
  if (lastOpen >= 0 && (lastOpen === 0 || text[lastOpen - 1] !== '\\')) {
    const startIdx =
      lastOpen > 0 && text[lastOpen - 1] === '!' ? lastOpen - 1 : lastOpen;
    const match = RE_LINK_TAIL.exec(text.slice(startIdx));
    if (match) {
      let matchLen = match[0].length;
      const replacement = match[1] ? '' : (match[2] ?? '');
      if (replacement === '') {
        let cutStart = startIdx;
        while (
          cutStart > 0 &&
          (text.charCodeAt(cutStart - 1) === 0x20 ||
            text.charCodeAt(cutStart - 1) === 0x09)
        ) {
          cutStart--;
        }
        matchLen += startIdx - cutStart;
      }
      heal = { strip: matchLen, append: replacement };
      text = text.slice(0, text.length - matchLen) + replacement;
    }
  }

  const n = text.length;
  let codeOpen: { len: number; start: number } | null = null;
  let wordHadEm = false;
  const stack: OpenDelimiter[] = [];
  let stars = 0;
  let underscores = 0;
  let tildes = 0;

  const push = (open: OpenDelimiter) => {
    if (open.ch === 0x2a) stars++;
    else if (open.ch === 0x5f) underscores++;
    else tildes++;
    stack.push(open);
  };

  const pop = () => {
    const open = stack.pop();
    if (!open) return;
    if (open.ch === 0x2a) stars--;
    else if (open.ch === 0x5f) underscores--;
    else tildes--;
  };

  const hasOpen = (ch: number) =>
    (ch === 0x2a ? stars : ch === 0x5f ? underscores : tildes) > 0;

  let i = 0;
  while (i < n) {
    const c = text.charCodeAt(i);
    if (codeOpen) {
      if (c === 0x60) {
        let run = 1;
        while (i + run < n && text.charCodeAt(i + run) === 0x60) run++;
        if (run === codeOpen.len) codeOpen = null;
        i += run;
      } else {
        i++;
      }
      continue;
    }
    if (isSpace(c)) {
      wordHadEm = false;
      i++;
      continue;
    }
    if (c === 0x5c /* \ */) {
      RE_INLINE_PAREN_MATH.lastIndex = i;
      const pm = RE_INLINE_PAREN_MATH.exec(text);
      if (pm) {
        i += pm[0].length;
        continue;
      }
      i += 2;
      continue;
    }
    if (c === 0x60 /* ` */) {
      let run = 1;
      while (i + run < n && text.charCodeAt(i + run) === 0x60) run++;
      codeOpen = { len: run, start: i };
      i += run;
      continue;
    }
    if (c === 0x24 /* $ */) {
      RE_INLINE_DOLLAR_MATH.lastIndex = i;
      const mm = RE_INLINE_DOLLAR_MATH.exec(text);
      if (mm) {
        i += mm[0].length;
        continue;
      }
      i++;
      continue;
    }
    if (c === 0x2a /* * */ || c === 0x5f /* _ */ || c === 0x7e /* ~ */) {
      let run = 1;
      while (i + run < n && text.charCodeAt(i + run) === c) run++;
      const prev = i > 0 ? text.charCodeAt(i - 1) : Number.NaN;
      const next = i + run < n ? text.charCodeAt(i + run) : Number.NaN;
      const isUnderscore = c === 0x5f;
      const isSingleStar = c === 0x2a && run === 1;
      const canOpen =
        !isSpace(next) &&
        !(isUnderscore && isAlnum(prev)) &&
        !(isSingleStar && isAlnum(prev) && !wordHadEm);
      const canClose = !isSpace(prev) && !(isUnderscore && isAlnum(next));
      if (c === 0x2a && (run >= 2 || canOpen)) wordHadEm = true;
      if (c === 0x7e && run > 2) {
        i += run;
        continue;
      }
      let remaining = run;
      if (canClose) {
        while (remaining > 0 && stack.length > 0) {
          if (stack[stack.length - 1].ch !== c) {
            if (!hasOpen(c)) break;
            while (stack.length > 0 && stack[stack.length - 1].ch !== c) {
              pop();
            }
          }
          const top = stack[stack.length - 1];
          if (!top) break;
          if (top.len <= remaining) {
            remaining -= top.len;
            pop();
          } else {
            top.len -= remaining;
            remaining = 0;
          }
        }
      }
      if (remaining > 0 && canOpen) {
        if (c === 0x7e) {
          push({
            ch: c,
            len: remaining,
            after: i + run,
            start: i + (run - remaining),
          });
        } else if (remaining >= 3) {
          push({
            ch: c,
            len: 2,
            after: i + run,
            start: i + (run - remaining),
          });
          push({
            ch: c,
            len: 1,
            after: i + run,
            start: i + (run - remaining) + 2,
          });
        } else {
          push({
            ch: c,
            len: remaining,
            after: i + run,
            start: i + (run - remaining),
          });
        }
      }
      i += run;
      continue;
    }
    i++;
  }

  let strip = 0;
  let append = '';
  let codeClosed = false;
  let nonSpaceEnd = rtrimLength(text);

  if (codeOpen) {
    if (codeOpen.start + codeOpen.len >= nonSpaceEnd) {
      let cut = codeOpen.start;
      while (
        cut > 0 &&
        (text.charCodeAt(cut - 1) === 0x20 || text.charCodeAt(cut - 1) === 0x09)
      ) {
        cut--;
      }
      strip = n - cut;
    } else {
      append += '`'.repeat(codeOpen.len);
      codeClosed = true;
    }
  }

  let limit = n - strip;
  if (nonSpaceEnd > limit) {
    nonSpaceEnd = limit;
    while (nonSpaceEnd > 0 && isSpace(text.charCodeAt(nonSpaceEnd - 1))) {
      nonSpaceEnd--;
    }
  }

  for (let s = stack.length - 1; s >= 0; s--) {
    const open = stack[s];
    if (open.start >= limit) continue;
    if (open.after >= nonSpaceEnd) {
      if (append === '') {
        let cut = open.start;
        while (
          cut > 0 &&
          (text.charCodeAt(cut - 1) === 0x20 ||
            text.charCodeAt(cut - 1) === 0x09)
        ) {
          cut--;
        }
        strip = n - cut;
        limit = cut;
        if (nonSpaceEnd > limit) {
          nonSpaceEnd = limit;
          while (nonSpaceEnd > 0 && isSpace(text.charCodeAt(nonSpaceEnd - 1))) {
            nonSpaceEnd--;
          }
        }
      }
      continue;
    }
    append += String.fromCharCode(open.ch).repeat(open.len);
  }

  if (append && !codeClosed) {
    let cut = n - strip;
    while (cut > 0 && isSpace(text.charCodeAt(cut - 1))) cut--;
    strip = n - cut;
  }

  // Hide a trailing bare inline opener run after whitespace (e.g. "Hello **") when nothing was appended.
  if (append === '' && strip === 0) {
    const bareOpenerMatch = text.match(/(?:^|[ \t])([*_~]{1,3})$/);
    if (bareOpenerMatch) {
      const markerRun = bareOpenerMatch[1];
      const runStart = text.length - markerRun.length;
      const prevBalanced = text
        .slice(0, runStart)
        .match(new RegExp(`\\${markerRun[0]}`, 'g'));
      if (!prevBalanced || prevBalanced.length % 2 === 0) {
        let cut = runStart;
        while (
          cut > 0 &&
          (text.charCodeAt(cut - 1) === 0x20 ||
            text.charCodeAt(cut - 1) === 0x09)
        ) {
          cut--;
        }
        strip = n - cut;
      }
    }
  }

  return compose(heal, { strip, append });
}

export function applyTailHeal(text: string, heal: TailHeal): string {
  if (heal.strip === 0 && heal.append === '') return text;
  const end = rtrimLength(text);
  return text.slice(0, Math.max(0, end - heal.strip)) + heal.append;
}

/**
 * Repairs incomplete inline markers at the tail of an actively streaming inline string.
 */
export function healStreamingInline(raw: string): string {
  if (!raw) return raw;
  const heal = computeTailHeal(raw);
  if (heal.strip === 0 && heal.append === '') return raw;
  return applyTailHeal(raw, heal);
}

/**
 * Holds back a trailing bare block opener (`#`, `-`, `*`, `+`, `1.`, `>`, ```` ``` ````, `---`, etc.)
 * until content or a newline arrives so headings/lists/code fences don't flash as raw text.
 */
export function splitPendingMarker(text: string): {
  visible: string;
  held: string;
} {
  const lastNl = text.lastIndexOf('\n');
  const lineStart = lastNl + 1;
  const tailLine = text.slice(lineStart);
  if (!tailLine || tailLine.length > 12) return { visible: text, held: '' };

  const trimmed = tailLine.trim();
  if (!trimmed) return { visible: text, held: '' };

  // Also hold trailing `-` / `=` candidate beneath an existing line.
  if (lastNl >= 0 && RE_SETEXT_CANDIDATE.test(tailLine)) {
    const prevLineStart = text.lastIndexOf('\n', lastNl - 1) + 1;
    const prevLine = text.slice(prevLineStart, lastNl);
    if (prevLine.trim().length > 0 && trimmed.length <= 2) {
      return { visible: text.slice(0, lineStart), held: tailLine };
    }
  }

  const first = trimmed.charCodeAt(0);
  if (
    first === 0x23 /* # */ ||
    first === 0x60 /* ` */ ||
    first === 0x7e /* ~ */ ||
    first === 0x2d /* - */ ||
    first === 0x2a /* * */ ||
    first === 0x5f /* _ */ ||
    first === 0x24 /* $ */ ||
    first === 0x3d /* = */
  ) {
    let allSame = true;
    for (let i = 1; i < trimmed.length; i++) {
      if (trimmed.charCodeAt(i) !== first) {
        allSame = false;
        break;
      }
    }
    if (allSame) {
      const maxHold =
        first === 0x23 ? 6 : first === 0x60 || first === 0x7e ? 3 : 2;
      if (trimmed.length <= maxHold) {
        return { visible: text.slice(0, lineStart), held: tailLine };
      }
    }
  }

  if (
    trimmed === '+' ||
    trimmed === '•' ||
    trimmed === '>' ||
    trimmed === '\\['
  ) {
    return { visible: text.slice(0, lineStart), held: tailLine };
  }

  let d = 0;
  while (
    d < trimmed.length &&
    trimmed.charCodeAt(d) >= 0x30 &&
    trimmed.charCodeAt(d) <= 0x39
  ) {
    d++;
  }
  if (d > 0) {
    const rest = trimmed.slice(d);
    if (rest === '' || rest === '.' || rest === ')') {
      return { visible: text.slice(0, lineStart), held: tailLine };
    }
  }

  return { visible: text, held: '' };
}

function endsInsideCodeFence(text: string): boolean {
  const lines = text.split(/\r?\n/);
  let inCode = false;
  let fenceChar = '';
  let fenceLen = 0;
  for (const line of lines) {
    const m = line.match(CODE_FENCE_LINE_RE);
    if (!m) continue;
    const token = m[1];
    if (!inCode) {
      inCode = true;
      fenceChar = token[0]!;
      fenceLen = token.length;
    } else if (token[0] === fenceChar && token.length >= fenceLen) {
      inCode = false;
      fenceChar = '';
      fenceLen = 0;
    }
  }
  return inCode;
}

const BLOCK_LINE_PREFIX_RE =
  /^( *(?:#{1,6} +|[-*+] +|\d+[.)] +|>+ ?|))([\s\S]*)$/;

/**
 * Applies streaming tail healing to a full markdown document buffer while `isPending === true`:
 * skips mutation when the tail is inside an open fenced code block, holds bare marker prefixes on
 * an incomplete final line, and closes dangling inline spans on the final visible line.
 */
export function healStreamingMarkdownDocument(text: string): string {
  if (!text) return text;
  if (endsInsideCodeFence(text)) {
    return text;
  }

  // Only hold bare markers / heal inline text when there is no trailing newline.
  if (text.endsWith('\n') || text.endsWith('\r')) {
    return text;
  }

  const { visible } = splitPendingMarker(text);
  if (!visible) return visible;
  if (visible.endsWith('\n') || visible.endsWith('\r')) {
    return visible;
  }

  const lastNl = visible.lastIndexOf('\n');
  const head = lastNl >= 0 ? visible.slice(0, lastNl + 1) : '';
  const tailLine = lastNl >= 0 ? visible.slice(lastNl + 1) : visible;

  // Preserve completed fence openers or horizontal rules untouched.
  if (
    CODE_FENCE_LINE_RE.test(tailLine) ||
    /^ *([-*_] *){3,} *$/.test(tailLine)
  ) {
    return visible;
  }

  const prefixMatch = tailLine.match(BLOCK_LINE_PREFIX_RE);
  if (!prefixMatch) {
    return head + healStreamingInline(tailLine);
  }
  const prefix = prefixMatch[1] ?? '';
  const body = prefixMatch[2] ?? '';
  if (!body) {
    return visible;
  }
  return head + prefix + healStreamingInline(body);
}
