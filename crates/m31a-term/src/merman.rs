//! Terminal Mermaid Diagram Engine (`merman`).
//!
//! Parses and renders Mermaid flowcharts and sequence diagrams directly to ANSI/Unicode
//! box-drawing characters for rich terminal visualization without headless browser dependencies.
//! Supports layered 2D DAG layout with multi-node branching, convergence, and routed edges.

use anyhow::{Result, anyhow};
use std::collections::{HashMap, HashSet, VecDeque};

/// Supported diagram kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagramKind {
    FlowchartTD,
    FlowchartLR,
    Sequence,
}

/// A parsed flowchart node.
#[derive(Debug, Clone)]
pub struct FlowNode {
    pub id: String,
    pub label: String,
    pub shape: NodeShape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeShape {
    Square,
    Rounded,
    Capsule,
    Diamond,
}

/// A directed edge between two flowchart nodes.
#[derive(Debug, Clone)]
pub struct FlowEdge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub style: EdgeStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeStyle {
    Normal,
    Dotted,
    Thick,
}

/// Detects if content contains a recognized Mermaid diagram.
pub fn is_mermaid_diagram(source: &str) -> bool {
    let trimmed = source.trim();
    trimmed.starts_with("graph ")
        || trimmed.starts_with("flowchart ")
        || trimmed.starts_with("sequenceDiagram")
}

/// Renders a Mermaid diagram source to a Unicode box-drawing string.
pub fn render_mermaid(source: &str) -> Result<String> {
    let trimmed = source.trim();
    if !is_mermaid_diagram(trimmed) {
        return Err(anyhow!("Not a recognized Mermaid diagram syntax"));
    }

    if trimmed.starts_with("sequenceDiagram") {
        render_sequence_diagram(trimmed)
    } else {
        render_flowchart_diagram(trimmed)
    }
}

/// Renders a Mermaid sequence diagram.
fn render_sequence_diagram(source: &str) -> Result<String> {
    let mut participants: Vec<String> = Vec::new();
    let mut messages: Vec<(String, String, String, bool)> = Vec::new(); // (from, to, text, is_dashed)

    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("sequenceDiagram") || line.starts_with("%%") {
            continue;
        }

        if let Some(rest) = line.strip_prefix("participant ") {
            let name = rest.split_whitespace().next().unwrap_or("").to_string();
            if !name.is_empty() && !participants.contains(&name) {
                participants.push(name);
            }
        } else if let Some((from_to, text)) = line.split_once(':') {
            let text = text.trim().to_string();
            let is_dashed = from_to.contains("-->>") || from_to.contains("-->");
            let delim = if from_to.contains("->>") {
                "->>"
            } else if from_to.contains("-->>") {
                "-->>"
            } else if from_to.contains("-->") {
                "-->"
            } else if from_to.contains("->") {
                "->"
            } else {
                continue;
            };

            if let Some((from, to)) = from_to.split_once(delim) {
                let from = from.trim().to_string();
                let to = to.trim().to_string();
                if !from.is_empty() && !participants.contains(&from) {
                    participants.push(from.clone());
                }
                if !to.is_empty() && !participants.contains(&to) {
                    participants.push(to.clone());
                }
                messages.push((from, to, text, is_dashed));
            }
        }
    }

    if participants.is_empty() {
        return Ok("(Empty sequence diagram)".to_string());
    }

    let mut out = Vec::new();
    let col_width: usize = 24;

    // Header boxes
    let mut top_line = String::new();
    let mut mid_line = String::new();
    let mut bot_line = String::new();

    for p in &participants {
        let p_len = p.len();
        let pad = (col_width.saturating_sub(p_len + 2)) / 2;
        let right_pad = col_width.saturating_sub(p_len + 2 + pad);

        top_line.push_str(&format!(
            "┌{}┐{}",
            "─".repeat(p_len),
            " ".repeat(pad + right_pad)
        ));
        mid_line.push_str(&format!("│{}│{}", p, " ".repeat(pad + right_pad)));
        bot_line.push_str(&format!(
            "└{}┘{}",
            "─".repeat(p_len),
            " ".repeat(pad + right_pad)
        ));
    }

    out.push(top_line);
    out.push(mid_line);
    out.push(bot_line);

    let lifelines = participants
        .iter()
        .map(|_| format!("│{}", " ".repeat(col_width - 1)))
        .collect::<Vec<_>>()
        .join("");
    out.push(lifelines.clone());

    // Messages
    for (from, to, text, is_dashed) in messages {
        let from_idx = participants.iter().position(|p| p == &from).unwrap_or(0);
        let to_idx = participants.iter().position(|p| p == &to).unwrap_or(0);

        if from_idx == to_idx {
            continue;
        }

        let total_cols = participants.len() * col_width;
        let mut row = vec![' '; total_cols];

        for i in 0..participants.len() {
            row[i * col_width] = '│';
        }

        let min_idx = from_idx.min(to_idx);
        let max_idx = from_idx.max(to_idx);
        let start_col = min_idx * col_width;
        let end_col = max_idx * col_width;

        let fill_char = if is_dashed { '┄' } else { '─' };
        row[(start_col + 1)..end_col].fill(fill_char);

        if from_idx < to_idx {
            row[end_col - 1] = '►';
        } else {
            row[start_col + 1] = '◄';
        }

        let label_start = start_col + 2;
        for (offset, ch) in text.chars().enumerate() {
            if label_start + offset < end_col - 1 {
                row[label_start + offset] = ch;
            }
        }

        let row_str: String = row.into_iter().collect();
        out.push(row_str.trim_end().to_string());
        out.push(lifelines.clone());
    }

    Ok(out.join("\n"))
}

/// Renders a Mermaid flowchart diagram.
fn render_flowchart_diagram(source: &str) -> Result<String> {
    let mut is_lr = false;
    let mut nodes: HashMap<String, FlowNode> = HashMap::new();
    let mut edges: Vec<FlowEdge> = Vec::new();

    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }

        if line.starts_with("graph ") || line.starts_with("flowchart ") {
            if line.contains("LR") || line.contains("RL") {
                is_lr = true;
            }
            continue;
        }

        // Check edge with optional label
        let arrow_patterns = ["-->", "-.->", "==>", "---"];
        for pat in arrow_patterns {
            if let Some((left, right)) = line.split_once(pat) {
                let from_node = parse_node_spec(left.trim());
                let (to_node, edge_label) = parse_right_spec(right.trim());

                let style = match pat {
                    "-.->" => EdgeStyle::Dotted,
                    "==>" => EdgeStyle::Thick,
                    _ => EdgeStyle::Normal,
                };

                nodes
                    .entry(from_node.id.clone())
                    .or_insert(from_node.clone());
                nodes.entry(to_node.id.clone()).or_insert(to_node.clone());

                edges.push(FlowEdge {
                    from: from_node.id,
                    to: to_node.id,
                    label: edge_label,
                    style,
                });
                break;
            }
        }
    }

    if nodes.is_empty() {
        return Ok("(Empty flowchart)".to_string());
    }

    if is_lr {
        render_flowchart_lr(&nodes, &edges)
    } else {
        render_flowchart_td(&nodes, &edges)
    }
}

fn parse_node_spec(raw: &str) -> FlowNode {
    let raw = raw.trim();
    if let Some((id, rest)) = raw.split_once("[\"") {
        let label = rest.strip_suffix("\"]").unwrap_or(rest).to_string();
        FlowNode {
            id: id.trim().to_string(),
            label,
            shape: NodeShape::Square,
        }
    } else if let Some((id, rest)) = raw.split_once('[') {
        let label = rest.strip_suffix(']').unwrap_or(rest).to_string();
        FlowNode {
            id: id.trim().to_string(),
            label,
            shape: NodeShape::Square,
        }
    } else if let Some((id, rest)) = raw.split_once("(\"") {
        let label = rest.strip_suffix("\")").unwrap_or(rest).to_string();
        FlowNode {
            id: id.trim().to_string(),
            label,
            shape: NodeShape::Rounded,
        }
    } else if let Some((id, rest)) = raw.split_once('(') {
        let label = rest.strip_suffix(')').unwrap_or(rest).to_string();
        FlowNode {
            id: id.trim().to_string(),
            label,
            shape: NodeShape::Rounded,
        }
    } else if let Some((id, rest)) = raw.split_once("{\"") {
        let label = rest.strip_suffix("\"}").unwrap_or(rest).to_string();
        FlowNode {
            id: id.trim().to_string(),
            label,
            shape: NodeShape::Diamond,
        }
    } else if let Some((id, rest)) = raw.split_once('{') {
        let label = rest.strip_suffix('}').unwrap_or(rest).to_string();
        FlowNode {
            id: id.trim().to_string(),
            label,
            shape: NodeShape::Diamond,
        }
    } else {
        FlowNode {
            id: raw.to_string(),
            label: raw.to_string(),
            shape: NodeShape::Rounded,
        }
    }
}

fn parse_right_spec(raw: &str) -> (FlowNode, Option<String>) {
    let raw = raw.trim();
    if let Some(rest) = raw.strip_prefix('|')
        && let Some((label, target)) = rest.split_once('|')
    {
        return (
            parse_node_spec(target.trim()),
            Some(label.trim().to_string()),
        );
    }
    (parse_node_spec(raw), None)
}

fn render_box(node: &FlowNode) -> [String; 3] {
    let label = &node.label;
    let width = label.chars().count() + 2;
    let (tl, tr, bl, br, h, v) = match node.shape {
        NodeShape::Rounded | NodeShape::Capsule => ('╭', '╮', '╰', '╯', '─', '│'),
        NodeShape::Diamond => ('◇', '◇', '◇', '◇', '─', '│'),
        NodeShape::Square => ('┌', '┐', '└', '┘', '─', '│'),
    };

    let top = format!("{}{}{}", tl, h.to_string().repeat(width), tr);
    let mid = format!("{} {} {}", v, label, v);
    let bot = format!("{}{}{}", bl, h.to_string().repeat(width), br);

    [top, mid, bot]
}

/// Computes topological ranks (layers) for DAG layout.
fn compute_layers(nodes: &HashMap<String, FlowNode>, edges: &[FlowEdge]) -> Vec<Vec<String>> {
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();

    for id in nodes.keys() {
        in_degree.insert(id.clone(), 0);
        adj.insert(id.clone(), Vec::new());
    }

    for edge in edges {
        *in_degree.entry(edge.to.clone()).or_insert(0) += 1;
        adj.entry(edge.from.clone())
            .or_default()
            .push(edge.to.clone());
    }

    let mut queue = VecDeque::new();
    let mut ranks: HashMap<String, usize> = HashMap::new();

    for (id, &deg) in &in_degree {
        if deg == 0 {
            queue.push_back((id.clone(), 0));
            ranks.insert(id.clone(), 0);
        }
    }

    // In case of cycles or no in-degree 0 nodes, pick first node
    if queue.is_empty()
        && !nodes.is_empty()
        && let Some(first_id) = nodes.keys().next()
    {
        queue.push_back((first_id.clone(), 0));
        ranks.insert(first_id.clone(), 0);
    }

    while let Some((curr, rank)) = queue.pop_front() {
        if let Some(neighbors) = adj.get(&curr) {
            for nxt in neighbors {
                let nxt_rank = rank + 1;
                let existing = ranks.get(nxt).copied().unwrap_or(0);
                if nxt_rank > existing {
                    ranks.insert(nxt.clone(), nxt_rank);
                    queue.push_back((nxt.clone(), nxt_rank));
                }
            }
        }
    }

    // Make sure all nodes have a rank
    for id in nodes.keys() {
        ranks.entry(id.clone()).or_insert(0);
    }

    let max_rank = ranks.values().max().copied().unwrap_or(0);
    let mut layers: Vec<Vec<String>> = vec![Vec::new(); max_rank + 1];

    for (id, rank) in ranks {
        layers[rank].push(id);
    }

    layers
}

/// Renders a top-down flowchart using a 2D layered DAG grid with routed connectors.
fn render_flowchart_td(nodes: &HashMap<String, FlowNode>, edges: &[FlowEdge]) -> Result<String> {
    let layers = compute_layers(nodes, edges);
    let mut out = Vec::new();

    // Map each node to its center X coordinate in its layer
    let mut node_centers: HashMap<String, usize> = HashMap::new();

    for (layer_idx, layer) in layers.iter().enumerate() {
        if layer.is_empty() {
            continue;
        }

        // Render layer node boxes side-by-side
        let mut row0 = String::new();
        let mut row1 = String::new();
        let mut row2 = String::new();

        let mut current_x = 4;
        let spacing = 6;

        for (node_idx, id) in layer.iter().enumerate() {
            if let Some(node) = nodes.get(id) {
                let box_lines = render_box(node);
                let box_width = box_lines[0].chars().count();
                let center_x = current_x + box_width / 2;
                node_centers.insert(id.clone(), center_x);

                if node_idx > 0 {
                    let pad = " ".repeat(spacing);
                    row0.push_str(&pad);
                    row1.push_str(&pad);
                    row2.push_str(&pad);
                } else {
                    let pad = " ".repeat(current_x);
                    row0.push_str(&pad);
                    row1.push_str(&pad);
                    row2.push_str(&pad);
                }

                row0.push_str(&box_lines[0]);
                row1.push_str(&box_lines[1]);
                row2.push_str(&box_lines[2]);

                current_x += box_width + spacing;
            }
        }

        out.push(row0);
        out.push(row1);
        out.push(row2);

        // If not the last layer, draw routed connectors to next layer
        if layer_idx + 1 < layers.len() {
            let next_layer: HashSet<&String> = layers[layer_idx + 1].iter().collect();
            let inter_edges: Vec<&FlowEdge> = edges
                .iter()
                .filter(|e| {
                    layer.contains(&e.from)
                        && (next_layer.contains(&e.to)
                            || layers.iter().skip(layer_idx + 1).any(|l| l.contains(&e.to)))
                })
                .collect();

            if !inter_edges.is_empty() {
                let canvas_width = current_x + 10;
                let mut line_drop = vec![' '; canvas_width];
                let mut line_bus = vec![' '; canvas_width];
                let mut line_arrow = vec![' '; canvas_width];

                for edge in &inter_edges {
                    let from_x = node_centers.get(&edge.from).copied().unwrap_or(8);
                    // Compute destination center if available, otherwise align with from_x
                    let to_x = node_centers.get(&edge.to).copied().unwrap_or(from_x);

                    let (v_char, h_char, arrow_char) = match edge.style {
                        EdgeStyle::Normal => ('│', '─', '▼'),
                        EdgeStyle::Dotted => ('┆', '┄', '▼'),
                        EdgeStyle::Thick => ('┃', '━', '🠇'),
                    };

                    if from_x == to_x {
                        line_drop[from_x] = v_char;
                        line_bus[from_x] = v_char;
                        line_arrow[from_x] = arrow_char;
                        if let Some(ref l) = edge.label {
                            let label_str = format!(" [{l}]");
                            for (offset, ch) in label_str.chars().enumerate() {
                                if from_x + 1 + offset < canvas_width {
                                    line_bus[from_x + 1 + offset] = ch;
                                }
                            }
                        }
                    } else {
                        let (min_x, max_x) = if from_x < to_x {
                            (from_x, to_x)
                        } else {
                            (to_x, from_x)
                        };
                        line_drop[from_x] = v_char;

                        for cell in &mut line_bus[min_x..=max_x] {
                            if *cell == ' ' {
                                *cell = h_char;
                            } else if *cell == v_char {
                                *cell = '┼';
                            }
                        }

                        // Set junctions
                        if from_x < to_x {
                            line_bus[from_x] = '┌';
                            line_bus[to_x] = '┐';
                        } else {
                            line_bus[from_x] = '┐';
                            line_bus[to_x] = '┌';
                        }

                        line_arrow[to_x] = arrow_char;

                        if let Some(ref l) = edge.label {
                            let label_str = format!(" [{l}]");
                            let mid = (min_x + max_x) / 2;
                            for (offset, ch) in label_str.chars().enumerate() {
                                if mid + offset < canvas_width {
                                    line_drop[mid + offset] = ch;
                                }
                            }
                        }
                    }
                }

                let s0: String = line_drop.into_iter().collect();
                let s1: String = line_bus.into_iter().collect();
                let s2: String = line_arrow.into_iter().collect();

                let t0 = s0.trim_end();
                if !t0.is_empty() {
                    out.push(t0.to_string());
                }
                let t1 = s1.trim_end();
                if !t1.is_empty() {
                    out.push(t1.to_string());
                }
                let t2 = s2.trim_end();
                if !t2.is_empty() {
                    out.push(t2.to_string());
                }
            }
        }
    }

    Ok(out.join("\n"))
}

/// Renders a left-to-right flowchart using a layered 2D column DAG layout.
fn render_flowchart_lr(nodes: &HashMap<String, FlowNode>, edges: &[FlowEdge]) -> Result<String> {
    let layers = compute_layers(nodes, edges);
    let mut out = Vec::new();

    // In LR layout, layers become columns
    // We compute box dimensions for each column
    let mut col_boxes: Vec<Vec<[String; 3]>> = Vec::new();

    for layer in &layers {
        let mut boxes_in_layer = Vec::new();
        for id in layer {
            if let Some(node) = nodes.get(id) {
                boxes_in_layer.push(render_box(node));
            }
        }
        col_boxes.push(boxes_in_layer);
    }

    for (col_idx, boxes) in col_boxes.iter().enumerate() {
        if col_idx + 1 < col_boxes.len() {
            let next_boxes = &col_boxes[col_idx + 1];
            let max_rows = boxes.len().max(next_boxes.len());

            for r in 0..max_rows {
                let left_box = boxes.get(r);
                let right_box = next_boxes.get(r);

                let dummy_box = [String::new(), String::new(), String::new()];
                let b1 = left_box.unwrap_or(&dummy_box);
                let b2 = right_box.unwrap_or(&dummy_box);

                let conn = " ────► ";
                let pad = " ".repeat(conn.chars().count());

                out.push(format!("{}  {}  {}", b1[0], pad, b2[0]));
                out.push(format!("{}  {}  {}", b1[1], conn, b2[1]));
                out.push(format!("{}  {}  {}", b1[2], pad, b2[2]));
                out.push(String::new());
            }
        }
    }

    // Fallback if single layer
    if out.is_empty() {
        for node in nodes.values() {
            let b = render_box(node);
            out.push(b[0].clone());
            out.push(b[1].clone());
            out.push(b[2].clone());
        }
    }

    Ok(out.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flowchart_td_rendering() {
        let source = "flowchart TD\nA[Start] --> B(Process)\nB --> C{Decision}";
        let res = render_mermaid(source).unwrap();
        assert!(res.contains("Start"));
        assert!(res.contains("Process"));
        assert!(res.contains("Decision"));
        assert!(res.contains("▼"));
    }

    #[test]
    fn test_flowchart_td_multi_node_branching_and_convergence() {
        let source = r#"
        flowchart TD
            A[Start] --> B(Path 1)
            A --> C(Path 2)
            B --> D[Merge]
            C --> D
        "#;
        let res = render_mermaid(source).unwrap();
        assert!(res.contains("Start"));
        assert!(res.contains("Path 1"));
        assert!(res.contains("Path 2"));
        assert!(res.contains("Merge"));
        assert!(res.contains("▼"));
    }

    #[test]
    fn test_flowchart_lr_rendering() {
        let source = "graph LR\nA[Frontend] -->|JSON-RPC| B[Backend]";
        let res = render_mermaid(source).unwrap();
        assert!(res.contains("Frontend"));
        assert!(res.contains("Backend"));
    }

    #[test]
    fn test_sequence_diagram_rendering() {
        let source = "sequenceDiagram\nAlice->>Bob: Hello\nBob-->>Alice: World";
        let res = render_mermaid(source).unwrap();
        assert!(res.contains("Alice"));
        assert!(res.contains("Bob"));
        assert!(res.contains("Hello"));
        assert!(res.contains("World"));
    }
}
