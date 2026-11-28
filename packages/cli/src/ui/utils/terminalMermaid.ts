/**
 * @license
 * Copyright 2026 Google LLC
 * SPDX-License-Identifier: Apache-2.0
 */

export type MermaidDiagramKind =
  | 'flowchart'
  | 'sequence'
  | 'state'
  | 'timeline'
  | 'gitGraph'
  | 'gantt';

export interface RenderedMermaidDiagram {
  kind: MermaidDiagramKind;
  lines: string[];
  width: number;
  height: number;
}

const HEADER_SKIP_RE = /^(?:%%|accTitle:|accDescr\b)/i;

function firstMeaningfulLines(source: string): string[] {
  return source
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0 && !HEADER_SKIP_RE.test(line));
}

export function detectMermaidDiagramKind(
  source: string,
): MermaidDiagramKind | undefined {
  const lines = firstMeaningfulLines(source);
  for (const line of lines) {
    if (line.startsWith('---')) continue;
    if (/^(?:graph|flowchart)(?:\s|$)/i.test(line)) return 'flowchart';
    if (/^sequenceDiagram\b/i.test(line)) return 'sequence';
    if (/^stateDiagram(?:-v2)?\b/i.test(line)) return 'state';
    if (/^timeline\b/i.test(line)) return 'timeline';
    if (/^gitGraph\b/i.test(line)) return 'gitGraph';
    if (/^gantt\b/i.test(line)) return 'gantt';
    break;
  }
  return undefined;
}

class TextGrid {
  private readonly rows: string[][];

  constructor(
    readonly width: number,
    readonly height: number,
  ) {
    this.rows = Array.from({ length: height }, () =>
      Array.from({ length: width }, () => ' '),
    );
  }

  set(x: number, y: number, ch: string): void {
    const ix = Math.trunc(x);
    const iy = Math.trunc(y);
    if (iy < 0 || iy >= this.height || ix < 0 || ix >= this.width) return;
    this.rows[iy][ix] = ch;
  }

  write(x: number, y: number, text: string): void {
    const iy = Math.trunc(y);
    if (iy < 0 || iy >= this.height) return;
    for (let i = 0; i < text.length; i++) {
      const ix = Math.trunc(x) + i;
      if (ix >= 0 && ix < this.width) {
        this.rows[iy][ix] = text[i]!;
      }
    }
  }

  drawBox(
    x: number,
    y: number,
    w: number,
    h: number,
    label: string,
    shape: 'rect' | 'round' | 'diamond' | 'circle' = 'rect',
  ): void {
    const tl = shape === 'round' || shape === 'circle' ? '╭' : '┌';
    const tr = shape === 'round' || shape === 'circle' ? '╮' : '┐';
    const bl = shape === 'round' || shape === 'circle' ? '╰' : '└';
    const br = shape === 'round' || shape === 'circle' ? '╯' : '┘';

    this.set(x, y, tl);
    this.set(x + w - 1, y, tr);
    this.set(x, y + h - 1, bl);
    this.set(x + w - 1, y + h - 1, br);

    for (let cx = x + 1; cx < x + w - 1; cx++) {
      this.set(cx, y, '─');
      this.set(cx, y + h - 1, '─');
    }
    for (let cy = y + 1; cy < y + h - 1; cy++) {
      this.set(x, cy, shape === 'diamond' ? '‹' : '│');
      this.set(x + w - 1, cy, shape === 'diamond' ? '›' : '│');
    }

    const innerWidth = Math.max(1, w - 4);
    const trimmed =
      label.length > innerWidth
        ? `${label.slice(0, Math.max(1, innerWidth - 1))}…`
        : label;
    const padLeft = Math.floor((w - trimmed.length) / 2);
    this.write(x + padLeft, y + Math.floor(h / 2), trimmed);
  }

  toLines(): string[] {
    const out = this.rows.map((r) => r.join('').replace(/\s+$/, ''));
    while (out.length > 0 && out[out.length - 1] === '') out.pop();
    while (out.length > 0 && out[0] === '') out.shift();
    return out;
  }
}

interface FlowNode {
  id: string;
  label: string;
  shape: 'rect' | 'round' | 'diamond' | 'circle';
  order: number;
}

interface FlowEdge {
  from: string;
  to: string;
  label?: string;
  dashed: boolean;
}

function parseNodeToken(
  raw: string,
  nodes: Map<string, FlowNode>,
  counter: { value: number },
): FlowNode | null {
  const token = raw.trim();
  if (!token) return null;

  let id = token;
  let label = token;
  let shape: FlowNode['shape'] = 'rect';

  const patterns: Array<{
    re: RegExp;
    shape: FlowNode['shape'];
  }> = [
    { re: /^([A-Za-z0-9_.-]+)\(\(([^)]+)\)\)$/, shape: 'circle' },
    { re: /^([A-Za-z0-9_.-]+)\(\[([^\]]+)\]\)$/, shape: 'round' },
    { re: /^([A-Za-z0-9_.-]+)\(([^)]+)\)$/, shape: 'round' },
    { re: /^([A-Za-z0-9_.-]+)\{([^}]+)\}$/, shape: 'diamond' },
    { re: /^([A-Za-z0-9_.-]+)\[([^\]]+)\]$/, shape: 'rect' },
  ];

  let matched = false;
  for (const p of patterns) {
    const m = token.match(p.re);
    if (m) {
      id = m[1]!;
      label = m[2].replace(/^["']|["']$/g, '').trim();
      shape = p.shape;
      matched = true;
      break;
    }
  }

  if (!matched) {
    const idOnly = token.match(/^([A-Za-z0-9_.-]+)$/);
    if (!idOnly) return null;
    id = idOnly[1]!;
    label = id;
  }

  const existing = nodes.get(id);
  if (existing) {
    if (matched) {
      existing.label = label;
      existing.shape = shape;
    }
    return existing;
  }

  const node: FlowNode = { id, label, shape, order: counter.value++ };
  nodes.set(id, node);
  return node;
}

function renderFlowchart(source: string, maxWidth: number): string[] | null {
  const lines = firstMeaningfulLines(source);
  const header = lines[0];
  if (!header) return null;
  const dirMatch = header.match(/^(?:graph|flowchart)\s+(TD|TB|BT|LR|RL)\b/i);
  const requestedDir = (dirMatch?.[1] ?? 'TD').toUpperCase();

  const nodes = new Map<string, FlowNode>();
  const edges: FlowEdge[] = [];
  const counter = { value: 0 };

  const edgeRe =
    /^(.+?)\s+(?:(--+>)|(===+>)|(-\.+->)|--+\s+([^->]+?)\s+--+>)\s+(.+)$/;

  for (let i = 1; i < lines.length; i++) {
    const line = lines[i].replace(/;$/, '').trim();
    if (
      !line ||
      line === 'end' ||
      /^(?:subgraph|classDef|class|style|linkStyle|click|direction)\b/i.test(
        line,
      )
    ) {
      continue;
    }

    // Handle piped label syntax: A -->|label| B
    const pipeEdge = line.match(
      /^(.+?)\s+(--+>|===+>|-\.+->)\|([^|]+)\|\s+(.+)$/,
    );
    if (pipeEdge) {
      const from = parseNodeToken(pipeEdge[1], nodes, counter);
      const to = parseNodeToken(pipeEdge[4], nodes, counter);
      if (from && to) {
        edges.push({
          from: from.id,
          to: to.id,
          label: pipeEdge[3].trim(),
          dashed: pipeEdge[2].includes('.'),
        });
      }
      continue;
    }

    const m = line.match(edgeRe);
    if (m) {
      const from = parseNodeToken(m[1], nodes, counter);
      const to = parseNodeToken(m[6], nodes, counter);
      if (from && to) {
        edges.push({
          from: from.id,
          to: to.id,
          label: m[5]?.trim(),
          dashed: Boolean(m[4]),
        });
      }
      continue;
    }

    // Standalone node declaration
    parseNodeToken(line, nodes, counter);
  }

  if (nodes.size === 0) return null;

  // Assign topological layers with cycle guard.
  const layerMap = new Map<string, number>();
  for (const id of nodes.keys()) layerMap.set(id, 0);
  for (let pass = 0; pass < nodes.size; pass++) {
    let changed = false;
    for (const edge of edges) {
      const fromLayer = layerMap.get(edge.from) ?? 0;
      const toLayer = layerMap.get(edge.to) ?? 0;
      if (fromLayer + 1 > toLayer && fromLayer + 1 < nodes.size) {
        layerMap.set(edge.to, fromLayer + 1);
        changed = true;
      }
    }
    if (!changed) break;
  }

  const layers: FlowNode[][] = [];
  for (const node of [...nodes.values()].sort((a, b) => a.order - b.order)) {
    const layerIdx = layerMap.get(node.id) ?? 0;
    if (!layers[layerIdx]) layers[layerIdx] = [];
    layers[layerIdx].push(node);
  }
  const compactLayers = layers.filter((l) => l && l.length > 0);
  if (compactLayers.length === 0) return null;

  const boxWidth = (n: FlowNode) =>
    Math.min(Math.max(n.label.length + 4, 7), Math.max(12, maxWidth - 2));

  // Estimate horizontal width to decide if LR fits, else fold to TD.
  const horizontalWidthNeeded = compactLayers.reduce(
    (sum, layer) => sum + Math.max(...layer.map((n) => boxWidth(n))) + 6,
    0,
  );
  const horizontal =
    (requestedDir === 'LR' || requestedDir === 'RL') &&
    horizontalWidthNeeded <= maxWidth;

  if (requestedDir === 'RL' || requestedDir === 'BT') {
    compactLayers.reverse();
  }

  if (horizontal) {
    const colWidths = compactLayers.map((l) =>
      Math.max(...l.map((n) => boxWidth(n))),
    );
    const maxRows = Math.max(...compactLayers.map((l) => l.length));
    const colGap = 5;
    const boxHeight = 3;
    const rowGap = 1;
    const totalW = Math.min(
      maxWidth,
      colWidths.reduce((a, b) => a + b, 0) +
        Math.max(0, compactLayers.length - 1) * colGap,
    );
    const totalH = maxRows * boxHeight + Math.max(0, maxRows - 1) * rowGap;
    const grid = new TextGrid(totalW, totalH);
    const coords = new Map<
      string,
      { x: number; y: number; w: number; h: number }
    >();

    let curX = 0;
    compactLayers.forEach((layer, cIdx) => {
      const w = colWidths[cIdx];
      layer.forEach((node, rIdx) => {
        const y = rIdx * (boxHeight + rowGap);
        grid.drawBox(curX, y, w, boxHeight, node.label, node.shape);
        coords.set(node.id, { x: curX, y, w, h: boxHeight });
      });
      curX += w + colGap;
    });

    for (const edge of edges) {
      const src = coords.get(edge.from);
      const dst = coords.get(edge.to);
      if (!src || !dst) continue;
      if (dst.x >= src.x + src.w) {
        const y = src.y + 1;
        const startX = src.x + src.w;
        const endX = dst.x - 1;
        for (let x = startX; x < endX; x++) {
          grid.set(x, y, edge.dashed ? '·' : '─');
        }
        grid.set(endX, y, '▶');
        if (edge.label) {
          const mid = Math.max(
            startX,
            Math.floor((startX + endX - edge.label.length) / 2),
          );
          if (y > 0) grid.write(mid, y - 1, edge.label);
        }
      }
    }

    return grid.toLines();
  }

  // Vertical TD layout
  const boxHeight = 3;
  const vGap = 2;
  const hGap = 3;
  const layerWidths = compactLayers.map(
    (l) =>
      l.reduce((sum, n) => sum + boxWidth(n), 0) +
      Math.max(0, l.length - 1) * hGap,
  );
  const totalW = Math.min(maxWidth, Math.max(12, ...layerWidths));
  const totalH =
    compactLayers.length * boxHeight +
    Math.max(0, compactLayers.length - 1) * vGap;
  const grid = new TextGrid(totalW, totalH);
  const coords = new Map<
    string,
    { x: number; y: number; w: number; h: number }
  >();

  let curY = 0;
  compactLayers.forEach((layer, lIdx) => {
    const layerW = layerWidths[lIdx];
    let curX = Math.max(0, Math.floor((totalW - layerW) / 2));
    for (const node of layer) {
      const w = boxWidth(node);
      if (curX + w <= totalW) {
        grid.drawBox(curX, curY, w, boxHeight, node.label, node.shape);
        coords.set(node.id, { x: curX, y: curY, w, h: boxHeight });
      }
      curX += w + hGap;
    }
    curY += boxHeight + vGap;
  });

  for (const edge of edges) {
    const src = coords.get(edge.from);
    const dst = coords.get(edge.to);
    if (!src || !dst) continue;
    const srcCx = src.x + Math.floor(src.w / 2);
    const dstCx = dst.x + Math.floor(dst.w / 2);
    if (dst.y >= src.y + src.h) {
      const topY = src.y + src.h;
      const botY = dst.y - 1;
      const midY = Math.floor((topY + botY) / 2);
      for (let y = topY; y < midY; y++) {
        grid.set(srcCx, y, edge.dashed ? '┆' : '│');
      }
      if (srcCx !== dstCx) {
        const minX = Math.min(srcCx, dstCx);
        const maxX = Math.max(srcCx, dstCx);
        for (let x = minX; x <= maxX; x++) {
          grid.set(x, midY, edge.dashed ? '·' : '─');
        }
        grid.set(srcCx, midY, srcCx < dstCx ? '└' : '┘');
        grid.set(dstCx, midY, srcCx < dstCx ? '┐' : '┌');
      } else {
        grid.set(dstCx, midY, edge.dashed ? '┆' : '│');
      }
      for (let y = midY + 1; y < botY; y++) {
        grid.set(dstCx, y, edge.dashed ? '┆' : '│');
      }
      grid.set(dstCx, botY, '▼');
      if (edge.label) {
        const labelX = Math.min(
          Math.max(0, dstCx + 2),
          Math.max(0, totalW - edge.label.length),
        );
        grid.write(labelX, midY, edge.label);
      }
    } else if (src.y === dst.y && dst.x > src.x + src.w) {
      const y = src.y + 1;
      for (let x = src.x + src.w; x < dst.x - 1; x++) {
        grid.set(x, y, edge.dashed ? '·' : '─');
      }
      grid.set(dst.x - 1, y, '▶');
    }
  }

  return grid.toLines();
}

function renderSequence(source: string, maxWidth: number): string[] | null {
  const lines = firstMeaningfulLines(source);
  const participants: Array<{ id: string; label: string }> = [];
  const participantSet = new Set<string>();
  const messages: Array<{
    from: string;
    to: string;
    label: string;
    dashed: boolean;
  }> = [];

  const ensureParticipant = (id: string, label?: string) => {
    const clean = id.trim();
    if (!clean) return;
    if (!participantSet.has(clean)) {
      participantSet.add(clean);
      participants.push({ id: clean, label: label ?? clean });
    } else if (label) {
      const p = participants.find((item) => item.id === clean);
      if (p) p.label = label;
    }
  };

  for (let i = 1; i < lines.length; i++) {
    const line = lines[i];
    const pMatch = line.match(
      /^(?:participant|actor)\s+([A-Za-z0-9_.-]+)(?:\s+as\s+(.+))?$/i,
    );
    if (pMatch) {
      ensureParticipant(pMatch[1], pMatch[2]?.trim());
      continue;
    }
    const msgMatch = line.match(
      /^([A-Za-z0-9_.-]+)\s*(--?>>?|--?[x)]|->)\s*([A-Za-z0-9_.-]+)\s*:\s*(.*)$/,
    );
    if (msgMatch) {
      const from = msgMatch[1];
      const arrow = msgMatch[2];
      const to = msgMatch[3];
      const label = msgMatch[4].trim();
      ensureParticipant(from);
      ensureParticipant(to);
      messages.push({
        from,
        to,
        label,
        dashed: arrow.startsWith('--'),
      });
    }
  }

  if (participants.length === 0) return null;

  const boxWidths = participants.map((p) =>
    Math.min(Math.max(p.label.length + 4, 7), 22),
  );
  const colGap = Math.max(
    4,
    Math.min(
      14,
      Math.floor(
        (maxWidth - boxWidths.reduce((a, b) => a + b, 0)) /
          Math.max(1, participants.length - 1),
      ),
    ),
  );
  const centers: number[] = [];
  let cursor = 0;
  participants.forEach((_, idx) => {
    const w = boxWidths[idx];
    centers.push(cursor + Math.floor(w / 2));
    cursor += w + colGap;
  });

  const totalW = Math.min(maxWidth, Math.max(20, cursor - colGap));
  const rowPerMsg = 2;
  const totalH = 4 + Math.max(1, messages.length * rowPerMsg);
  const grid = new TextGrid(totalW, totalH);

  let xPos = 0;
  participants.forEach((p, idx) => {
    const w = boxWidths[idx];
    if (xPos + w <= totalW) {
      grid.drawBox(xPos, 0, w, 3, p.label, 'rect');
    }
    const cx = centers[idx];
    for (let y = 3; y < totalH; y++) {
      grid.set(cx, y, '│');
    }
    xPos += w + colGap;
  });

  messages.forEach((msg, mIdx) => {
    const fromIdx = participants.findIndex((p) => p.id === msg.from);
    const toIdx = participants.findIndex((p) => p.id === msg.to);
    if (fromIdx < 0 || toIdx < 0) return;
    const fromX = centers[fromIdx];
    const toX = centers[toIdx];
    const labelY = 3 + mIdx * rowPerMsg;
    const arrowY = labelY + 1;

    if (fromX === toX) {
      grid.write(fromX, labelY, `↻ ${msg.label}`);
      return;
    }

    const minX = Math.min(fromX, toX);
    const maxX = Math.max(fromX, toX);
    const avail = Math.max(1, maxX - minX - 2);
    const lbl =
      msg.label.length > avail
        ? `${msg.label.slice(0, Math.max(1, avail - 1))}…`
        : msg.label;
    grid.write(minX + 2, labelY, lbl);

    const lineCh = msg.dashed ? '·' : '─';
    for (let x = minX + 1; x < maxX; x++) {
      grid.set(x, arrowY, lineCh);
    }
    if (toX > fromX) {
      grid.set(toX - 1, arrowY, '▶');
    } else {
      grid.set(toX + 1, arrowY, '◀');
    }
  });

  return grid.toLines();
}

function renderState(source: string, maxWidth: number): string[] | null {
  // Reuse the flowchart layout by normalizing state transitions into nodes & edges.
  const lines = firstMeaningfulLines(source);
  const transformed: string[] = ['flowchart TD'];
  const aliases = new Map<string, string>();
  let startCount = 0;

  for (let i = 1; i < lines.length; i++) {
    const line = lines[i];
    const aliasMatch = line.match(
      /^state\s+"([^"]+)"\s+as\s+([A-Za-z0-9_.-]+)/i,
    );
    if (aliasMatch) {
      aliases.set(aliasMatch[2], aliasMatch[1]);
      continue;
    }
    const descMatch = line.match(/^([A-Za-z0-9_.-]+)\s*:\s*(.+)$/);
    if (descMatch && !line.includes('-->')) {
      aliases.set(descMatch[1], `${descMatch[1]}: ${descMatch[2]}`);
      continue;
    }
    const transMatch = line.match(
      /^(\[\*\]|[A-Za-z0-9_.-]+)\s*-->\s*(\[\*\]|[A-Za-z0-9_.-]+)(?:\s*:\s*(.+))?$/,
    );
    if (transMatch) {
      const fromRaw =
        transMatch[1] === '[*]' ? `START_${++startCount}((●))` : transMatch[1];
      const toRaw =
        transMatch[2] === '[*]' ? `END_${++startCount}((◉))` : transMatch[2];
      const fromNode = aliases.has(fromRaw)
        ? `${fromRaw}(${aliases.get(fromRaw)!})`
        : fromRaw;
      const toNode = aliases.has(toRaw)
        ? `${toRaw}(${aliases.get(toRaw)!})`
        : toRaw;
      if (transMatch[3]) {
        transformed.push(`${fromNode} -->|${transMatch[3].trim()}| ${toNode}`);
      } else {
        transformed.push(`${fromNode} --> ${toNode}`);
      }
    }
  }

  return renderFlowchart(transformed.join('\n'), maxWidth);
}

function renderTimeline(source: string, maxWidth: number): string[] | null {
  const lines = firstMeaningfulLines(source);
  let title = '';
  const items: Array<{ period: string; events: string[] }> = [];

  for (let i = 1; i < lines.length; i++) {
    const line = lines[i];
    const tMatch = line.match(/^title\s+(.+)$/i);
    if (tMatch) {
      title = tMatch[1].trim();
      continue;
    }
    if (/^section\s+/i.test(line)) {
      items.push({
        period: `[${line.replace(/^section\s+/i, '').trim()}]`,
        events: [],
      });
      continue;
    }
    const colonIdx = line.indexOf(':');
    if (colonIdx > 0) {
      const period = line.slice(0, colonIdx).trim();
      const eventPart = line.slice(colonIdx + 1).trim();
      items.push({
        period,
        events: eventPart
          .split(':')
          .map((e) => e.trim())
          .filter(Boolean),
      });
    } else if (items.length > 0 && line.startsWith(':')) {
      items[items.length - 1].events.push(line.slice(1).trim());
    }
  }

  if (items.length === 0) return null;
  const out: string[] = [];
  if (title) out.push(`┌─ ${title} ─┐`.slice(0, maxWidth));
  for (const item of items) {
    const eventsText = item.events.join(' · ');
    const line = eventsText
      ? `├─ ${item.period} ── ${eventsText}`
      : `├─ ${item.period}`;
    out.push(
      line.length > maxWidth
        ? `${line.slice(0, Math.max(1, maxWidth - 1))}…`
        : line,
    );
  }
  return out;
}

function renderGitGraph(source: string, maxWidth: number): string[] | null {
  const lines = firstMeaningfulLines(source);
  const out: string[] = [];
  const branches = ['main'];
  let activeBranch = 'main';
  let commitNum = 0;

  for (let i = 1; i < lines.length; i++) {
    const line = lines[i];
    if (/^commit\b/i.test(line)) {
      commitNum++;
      const idMatch = line.match(/id:\s*"([^"]+)"/i);
      const tagMatch = line.match(/tag:\s*"([^"]+)"/i);
      const laneIdx = Math.max(0, branches.indexOf(activeBranch));
      const prefix = `${'│ '.repeat(laneIdx)}●`;
      const desc = `${idMatch?.[1] ?? `c${commitNum}`}${tagMatch ? ` [${tagMatch[1]}]` : ''} (${activeBranch})`;
      const row = `${prefix} ${desc}`;
      out.push(
        row.length > maxWidth
          ? `${row.slice(0, Math.max(1, maxWidth - 1))}…`
          : row,
      );
    } else if (/^branch\s+([A-Za-z0-9_./-]+)/i.test(line)) {
      const name = line.match(/^branch\s+([A-Za-z0-9_./-]+)/i)![1];
      if (!branches.includes(name)) branches.push(name);
      activeBranch = name;
      out.push(
        `${'│ '.repeat(Math.max(0, branches.length - 1))}╭─ branch ${name}`.slice(
          0,
          maxWidth,
        ),
      );
    } else if (/^(?:checkout|switch)\s+([A-Za-z0-9_./-]+)/i.test(line)) {
      activeBranch = line.match(
        /^(?:checkout|switch)\s+([A-Za-z0-9_./-]+)/i,
      )![1]!;
    } else if (/^merge\s+([A-Za-z0-9_./-]+)/i.test(line)) {
      const merged = line.match(/^merge\s+([A-Za-z0-9_./-]+)/i)![1];
      out.push(`├─◀ merge ${merged} → ${activeBranch}`.slice(0, maxWidth));
    }
  }

  return out.length > 0 ? out : null;
}

function renderGantt(source: string, maxWidth: number): string[] | null {
  const lines = firstMeaningfulLines(source);
  let title = '';
  const tasks: Array<{ name: string; section: string }> = [];
  let section = '';

  for (let i = 1; i < lines.length; i++) {
    const line = lines[i];
    if (/^title\s+/i.test(line)) {
      title = line.replace(/^title\s+/i, '').trim();
    } else if (/^section\s+/i.test(line)) {
      section = line.replace(/^section\s+/i, '').trim();
    } else if (
      /^(?:dateFormat|axisFormat|excludes|todayMarker)\b/i.test(line)
    ) {
      continue;
    } else if (line.includes(':')) {
      const name = line.slice(0, line.indexOf(':')).trim();
      if (name) tasks.push({ name, section });
    }
  }

  if (tasks.length === 0) return null;
  const labelW = Math.min(24, Math.max(8, ...tasks.map((t) => t.name.length)));
  const barArea = Math.max(8, Math.min(36, maxWidth - labelW - 4));
  const stepWidth = Math.max(
    3,
    Math.floor(barArea / Math.max(1, tasks.length)),
  );

  const out: string[] = [];
  if (title) out.push(title.slice(0, maxWidth));
  let lastSection = '';
  tasks.forEach((task, idx) => {
    if (task.section && task.section !== lastSection) {
      out.push(`[${task.section}]`.slice(0, maxWidth));
      lastSection = task.section;
    }
    const lbl = task.name.slice(0, labelW).padEnd(labelW, ' ');
    const offset = Math.min(
      barArea - 3,
      idx * Math.max(1, Math.floor(stepWidth / 2)),
    );
    const barLen = Math.min(barArea - offset, Math.max(3, stepWidth));
    const bar = `${' '.repeat(offset)}${'█'.repeat(barLen)}`;
    out.push(`${lbl} │ ${bar}`.slice(0, maxWidth));
  });

  return out;
}

/**
 * Renders supported Mermaid diagrams (`flowchart`/`graph`, `sequenceDiagram`,
 * `stateDiagram`, `timeline`, `gitGraph`, `gantt`) into clean Unicode/ASCII lines
 * sized for terminal output. Returns null when syntax cannot be parsed yet.
 */
export function renderMermaidToLines(
  source: string,
  maxWidth: number,
): RenderedMermaidDiagram | null {
  const kind = detectMermaidDiagramKind(source);
  if (!kind) return null;
  const clampedWidth = Math.max(24, Math.trunc(maxWidth));

  let lines: string[] | null = null;
  switch (kind) {
    case 'flowchart':
      lines = renderFlowchart(source, clampedWidth);
      break;
    case 'sequence':
      lines = renderSequence(source, clampedWidth);
      break;
    case 'state':
      lines = renderState(source, clampedWidth);
      break;
    case 'timeline':
      lines = renderTimeline(source, clampedWidth);
      break;
    case 'gitGraph':
      lines = renderGitGraph(source, clampedWidth);
      break;
    case 'gantt':
      lines = renderGantt(source, clampedWidth);
      break;
    default:
      lines = null;
      break;
  }

  if (!lines || lines.length === 0) return null;
  const width = Math.max(...lines.map((l) => l.length));
  return {
    kind,
    lines,
    width,
    height: lines.length,
  };
}
