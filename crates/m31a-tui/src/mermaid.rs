//! Mermaid diagram parser and terminal ASCII/Unicode graph renderer for flowcharts and sequences.
//!
//! Parses simple Mermaid diagram syntax and draws structured visual diagrams within terminal viewports.

use std::collections::{HashMap, HashSet};
use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub enum MermaidError {
    EmptySource,
    UnsupportedDiagramType(String),
    ExceedsMaxWidth(usize, usize),
}

impl fmt::Display for MermaidError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySource => write!(f, "Empty mermaid source diagram"),
            Self::UnsupportedDiagramType(t) => write!(f, "Unsupported mermaid diagram type: {}", t),
            Self::ExceedsMaxWidth(w, max) => write!(
                f,
                "Rendered diagram width {} exceeds max display width {}",
                w, max
            ),
        }
    }
}

impl std::error::Error for MermaidError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowDirection {
    TopDown,
    LeftRight,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeShape {
    Rectangle,
    Round,
    Database,
    Decision,
}

#[derive(Debug, Clone)]
pub struct FlowNode {
    pub id: String,
    pub label: String,
    pub shape: NodeShape,
}

#[derive(Debug, Clone)]
pub struct FlowEdge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub dashed: bool,
    pub thick: bool,
}

#[derive(Debug, Clone)]
pub struct SequenceParticipant {
    pub id: String,
    pub display: String,
}

#[derive(Debug, Clone)]
pub struct SequenceMessage {
    pub from: String,
    pub to: String,
    pub text: String,
    pub dashed: bool,
    pub arrow: bool,
}

/// Renders a Mermaid diagram source into clean terminal Unicode art.
pub fn render(source: &str, max_width: usize) -> Result<String, MermaidError> {
    let lines: Vec<&str> = source
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("%%"))
        .collect();

    if lines.is_empty() {
        return Err(MermaidError::EmptySource);
    }

    let header = lines[0].to_ascii_lowercase();

    let output = if header.starts_with("graph") || header.starts_with("flowchart") {
        render_flowchart(&lines, max_width)?
    } else if header.starts_with("sequencediagram") {
        render_sequence(&lines[1..], max_width)?
    } else if header.starts_with("statediagram") {
        render_state_diagram(&lines[1..], max_width)?
    } else {
        return Err(MermaidError::UnsupportedDiagramType(lines[0].to_string()));
    };

    Ok(output)
}

/// Parses and renders flowcharts (TopDown and LeftRight).
fn render_flowchart(lines: &[&str], max_width: usize) -> Result<String, MermaidError> {
    let header = lines[0].to_ascii_uppercase();
    let direction = if header.contains("LR") {
        FlowDirection::LeftRight
    } else {
        FlowDirection::TopDown
    };

    let mut nodes: HashMap<String, FlowNode> = HashMap::new();
    let mut edges: Vec<FlowEdge> = Vec::new();

    for &line in &lines[1..] {
        if line.is_empty() || line.starts_with("subgraph") || line == "end" {
            continue;
        }

        // Parse edge: e.g. A["Client"] -->|HTTP| B["Server"] or A --> B
        if let Some(edge) = parse_flow_edge(line, &mut nodes) {
            edges.push(edge);
        } else if let Some(node) = parse_single_node(line) {
            nodes.entry(node.id.clone()).or_insert(node);
        }
    }

    let result = match direction {
        FlowDirection::TopDown => render_flowchart_td(&nodes, &edges),
        FlowDirection::LeftRight => render_flowchart_lr(&nodes, &edges),
    };

    let line_width = result.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    if max_width > 0 && line_width > max_width {
        return Err(MermaidError::ExceedsMaxWidth(line_width, max_width));
    }

    Ok(result)
}

fn parse_single_node(line: &str) -> Option<FlowNode> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Node shapes:
    // A[(Database)]
    if let (Some(id_end), Some(start), Some(end)) =
        (trimmed.find("[("), trimmed.find("[("), trimmed.rfind(")]"))
    {
        let id = trimmed[..id_end].trim().to_string();
        let label = trimmed[start + 2..end].trim().trim_matches('"').to_string();
        return Some(FlowNode {
            id,
            label,
            shape: NodeShape::Database,
        });
    }
    // A{Decision}
    if let (Some(id_end), Some(start), Some(end)) =
        (trimmed.find('{'), trimmed.find('{'), trimmed.rfind('}'))
    {
        let id = trimmed[..id_end].trim().to_string();
        let label = trimmed[start + 1..end].trim().trim_matches('"').to_string();
        return Some(FlowNode {
            id,
            label,
            shape: NodeShape::Decision,
        });
    }
    // A([Round])
    if let (Some(id_end), Some(start), Some(end)) =
        (trimmed.find("(["), trimmed.find("(["), trimmed.rfind("])"))
    {
        let id = trimmed[..id_end].trim().to_string();
        let label = trimmed[start + 2..end].trim().trim_matches('"').to_string();
        return Some(FlowNode {
            id,
            label,
            shape: NodeShape::Round,
        });
    }
    // A["Label"] or A[Label]
    if let (Some(id_end), Some(start), Some(end)) =
        (trimmed.find('['), trimmed.find('['), trimmed.rfind(']'))
    {
        let id = trimmed[..id_end].trim().to_string();
        let label = trimmed[start + 1..end].trim().trim_matches('"').to_string();
        return Some(FlowNode {
            id,
            label,
            shape: NodeShape::Rectangle,
        });
    }

    // Plain ID
    if !trimmed.contains(' ') {
        return Some(FlowNode {
            id: trimmed.to_string(),
            label: trimmed.to_string(),
            shape: NodeShape::Rectangle,
        });
    }

    None
}

fn parse_flow_edge(line: &str, nodes: &mut HashMap<String, FlowNode>) -> Option<FlowEdge> {
    // Recognize connectors: -->, ==>, -.->, -- "label" -->, -->|label|
    let connectors = ["-->|", "-.->", "==>", "-->"];
    for conn in connectors {
        if let Some(idx) = line.find(conn) {
            let left_part = line[..idx].trim();
            let mut right_part = line[idx + conn.len()..].trim();
            let mut edge_label = None;

            if conn == "-->|"
                && let Some(pipe_end) = right_part.find('|')
            {
                edge_label = Some(right_part[..pipe_end].trim().to_string());
                right_part = right_part[pipe_end + 1..].trim();
            }

            let from_node = parse_single_node(left_part).unwrap_or_else(|| FlowNode {
                id: left_part.to_string(),
                label: left_part.to_string(),
                shape: NodeShape::Rectangle,
            });
            let to_node = parse_single_node(right_part).unwrap_or_else(|| FlowNode {
                id: right_part.to_string(),
                label: right_part.to_string(),
                shape: NodeShape::Rectangle,
            });

            let from_id = from_node.id.clone();
            let to_id = to_node.id.clone();

            nodes.entry(from_id.clone()).or_insert(from_node);
            nodes.entry(to_id.clone()).or_insert(to_node);

            let dashed = conn == "-.->";
            let thick = conn == "==>";

            return Some(FlowEdge {
                from: from_id,
                to: to_id,
                label: edge_label,
                dashed,
                thick,
            });
        }
    }
    None
}

fn render_box(node: &FlowNode) -> Vec<String> {
    let label = &node.label;
    let len = label.chars().count();
    let pad = 2;
    let inner_width = len + (pad * 2);

    match node.shape {
        NodeShape::Database => vec![
            format!(" ⌠{}⌡ ", "─".repeat(inner_width - 2)),
            format!("│ {}{} │", " ".repeat(pad - 1), label),
            format!(" ⌡{}⌠ ", "─".repeat(inner_width - 2)),
        ],
        NodeShape::Decision => vec![format!("  ◇ {} ◇  ", label)],
        NodeShape::Round => vec![
            format!("╭{}╮", "─".repeat(inner_width)),
            format!("│ {}{} │", " ".repeat(pad - 1), label),
            format!("╰{}╯", "─".repeat(inner_width)),
        ],
        NodeShape::Rectangle => vec![
            format!("┌{}┐", "─".repeat(inner_width)),
            format!("│ {}{} │", " ".repeat(pad - 1), label),
            format!("└{}┘", "─".repeat(inner_width)),
        ],
    }
}

fn render_flowchart_td(nodes: &HashMap<String, FlowNode>, edges: &[FlowEdge]) -> String {
    // Topological sort or simple linear order
    let mut ordered_ids = Vec::new();
    let mut visited = HashSet::new();

    // Start with root nodes (nodes with 0 incoming edges)
    let targets: HashSet<&str> = edges.iter().map(|e| e.to.as_str()).collect();
    for id in nodes.keys() {
        if !targets.contains(id.as_str()) && visited.insert(id.clone()) {
            ordered_ids.push(id.clone());
        }
    }
    for edge in edges {
        if visited.insert(edge.to.clone()) {
            ordered_ids.push(edge.to.clone());
        }
    }
    for id in nodes.keys() {
        if visited.insert(id.clone()) {
            ordered_ids.push(id.clone());
        }
    }

    let mut out = Vec::new();
    for (i, id) in ordered_ids.iter().enumerate() {
        if let Some(node) = nodes.get(id) {
            let box_lines = render_box(node);
            out.extend(box_lines);

            if i < ordered_ids.len() - 1 {
                // Find edge to next node
                let next_id = &ordered_ids[i + 1];
                let edge = edges.iter().find(|e| e.from == *id && e.to == *next_id);
                let label_text = edge
                    .and_then(|e| e.label.as_ref())
                    .map(|l| format!(" [{}]", l))
                    .unwrap_or_default();
                out.push(format!("      │{}", label_text));
                out.push("      ▼".to_string());
            }
        }
    }

    out.join("\n")
}

fn render_flowchart_lr(nodes: &HashMap<String, FlowNode>, edges: &[FlowEdge]) -> String {
    // Horizontal rendering: Box1 ──▶ Box2 ──▶ Box3
    let mut ordered_ids = Vec::new();
    let mut visited = HashSet::new();

    let targets: HashSet<&str> = edges.iter().map(|e| e.to.as_str()).collect();
    for id in nodes.keys() {
        if !targets.contains(id.as_str()) && visited.insert(id.clone()) {
            ordered_ids.push(id.clone());
        }
    }
    for edge in edges {
        if visited.insert(edge.to.clone()) {
            ordered_ids.push(edge.to.clone());
        }
    }
    for id in nodes.keys() {
        if visited.insert(id.clone()) {
            ordered_ids.push(id.clone());
        }
    }

    let mut box_layers: Vec<(Vec<String>, Option<String>)> = Vec::new();
    for (i, id) in ordered_ids.iter().enumerate() {
        if let Some(node) = nodes.get(id) {
            let b = render_box(node);
            let next_label = if i < ordered_ids.len() - 1 {
                let next_id = &ordered_ids[i + 1];
                let edge = edges.iter().find(|e| e.from == *id && e.to == *next_id);
                edge.and_then(|e| e.label.clone())
            } else {
                None
            };
            box_layers.push((b, next_label));
        }
    }

    if box_layers.is_empty() {
        return String::new();
    }

    let height = box_layers.iter().map(|(b, _)| b.len()).max().unwrap_or(3);
    let mut lines = vec![String::new(); height];

    for (idx, (box_lines, connector_label)) in box_layers.iter().enumerate() {
        for row in 0..height {
            if row < box_lines.len() {
                lines[row].push_str(&box_lines[row]);
            } else {
                lines[row].push_str(&" ".repeat(box_lines[0].chars().count()));
            }
        }

        if idx < box_layers.len() - 1 {
            let connector = if let Some(lbl) = connector_label {
                format!(" ──[{}]──▶ ", lbl)
            } else {
                " ─────▶ ".to_string()
            };
            let mid = height / 2;
            for (row, line) in lines.iter_mut().enumerate().take(height) {
                if row == mid {
                    line.push_str(&connector);
                } else {
                    line.push_str(&" ".repeat(connector.chars().count()));
                }
            }
        }
    }

    lines.join("\n")
}

/// Parses and renders sequence diagrams.
fn render_sequence(lines: &[&str], max_width: usize) -> Result<String, MermaidError> {
    let mut participants: Vec<SequenceParticipant> = Vec::new();
    let mut messages: Vec<SequenceMessage> = Vec::new();

    for &line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(rest) = trimmed
            .strip_prefix("participant ")
            .or_else(|| trimmed.strip_prefix("actor "))
        {
            let parts: Vec<&str> = rest.split(" as ").collect();
            let id = parts[0].trim().to_string();
            let display = parts
                .get(1)
                .map(|d| d.trim().to_string())
                .unwrap_or_else(|| id.clone());
            if !participants.iter().any(|p| p.id == id) {
                participants.push(SequenceParticipant { id, display });
            }
            continue;
        }

        // Messages: A->>B: text or A-->>B: text
        let is_dashed = trimmed.contains("-->>");
        let conn = if is_dashed { "-->>" } else { "->>" };
        if let Some(idx) = trimmed.find(conn) {
            let from = trimmed[..idx].trim().to_string();
            let right = &trimmed[idx + conn.len()..];
            let (to, text) = if let Some(colon) = right.find(':') {
                (
                    right[..colon].trim().to_string(),
                    right[colon + 1..].trim().to_string(),
                )
            } else {
                (right.trim().to_string(), String::new())
            };

            if !participants.iter().any(|p| p.id == from) {
                participants.push(SequenceParticipant {
                    id: from.clone(),
                    display: from.clone(),
                });
            }
            if !participants.iter().any(|p| p.id == to) {
                participants.push(SequenceParticipant {
                    id: to.clone(),
                    display: to.clone(),
                });
            }

            messages.push(SequenceMessage {
                from,
                to,
                text,
                dashed: is_dashed,
                arrow: true,
            });
        }
    }

    if participants.is_empty() {
        return Err(MermaidError::EmptySource);
    }

    // Calculate column positions
    let col_width: usize = 24;
    let mut header_boxes = Vec::new();
    let mut header_row1 = String::new();
    let mut header_row2 = String::new();
    let mut header_row3 = String::new();

    for p in &participants {
        let name = &p.display;
        let pad = (col_width.saturating_sub(name.chars().count() + 2)) / 2;
        let top = format!("┌{}┐", "─".repeat(col_width - 2));
        let mid = format!("│ {}{} │", " ".repeat(pad), name);
        let bot = format!("└{}┘", "─".repeat(col_width - 2));
        header_boxes.push((top, mid, bot));
    }

    for (top, mid, bot) in &header_boxes {
        header_row1.push_str(top);
        header_row1.push_str("   ");
        header_row2.push_str(mid);
        header_row2.push_str("   ");
        header_row3.push_str(bot);
        header_row3.push_str("   ");
    }

    let mut out = vec![header_row1, header_row2, header_row3];

    // Compute column centers
    let mut centers = Vec::new();
    for i in 0..participants.len() {
        let center = (i * (col_width + 3)) + (col_width / 2);
        centers.push(center);
    }

    let total_width = centers.last().copied().unwrap_or(0) + (col_width / 2) + 3;

    // Draw idle lifeline
    let make_lifeline = || {
        let mut line = vec![' '; total_width];
        for &c in &centers {
            if c < total_width {
                line[c] = '│';
            }
        }
        line.into_iter().collect::<String>()
    };

    out.push(make_lifeline());

    for msg in &messages {
        let from_idx = participants
            .iter()
            .position(|p| p.id == msg.from)
            .unwrap_or(0);
        let to_idx = participants
            .iter()
            .position(|p| p.id == msg.to)
            .unwrap_or(0);

        let from_c = centers[from_idx];
        let to_c = centers[to_idx];

        let min_c = from_c.min(to_c);
        let max_c = from_c.max(to_c);

        // Text line above arrow
        let mut text_line: Vec<char> = make_lifeline().chars().collect();
        let label = format!(" {} ", msg.text);
        let text_start = min_c + 2;
        for (offset, ch) in label.chars().enumerate() {
            if text_start + offset < max_c && text_start + offset < text_line.len() {
                text_line[text_start + offset] = ch;
            }
        }
        out.push(text_line.into_iter().collect());

        // Arrow line
        let mut arrow_line: Vec<char> = make_lifeline().chars().collect();
        let fill = if msg.dashed { '┄' } else { '─' };
        for x in (min_c + 1)..max_c {
            if x < arrow_line.len() {
                arrow_line[x] = fill;
            }
        }

        if from_c < to_c {
            // Left to right: ────▶
            if to_c < arrow_line.len() {
                arrow_line[to_c] = '▶';
            }
        } else {
            // Right to left: ◀────
            if to_c < arrow_line.len() {
                arrow_line[to_c] = '◀';
            }
        }
        out.push(arrow_line.into_iter().collect());
        out.push(make_lifeline());
    }

    let result = out.join("\n");
    let line_width = result.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    if max_width > 0 && line_width > max_width {
        return Err(MermaidError::ExceedsMaxWidth(line_width, max_width));
    }

    Ok(result)
}

/// Parses and renders state diagrams.
fn render_state_diagram(lines: &[&str], max_width: usize) -> Result<String, MermaidError> {
    let mut states: Vec<String> = Vec::new();
    let mut transitions: Vec<(String, String, Option<String>)> = Vec::new();

    for &line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(idx) = trimmed.find("-->") {
            let from = trimmed[..idx].trim().to_string();
            let right = &trimmed[idx + 3..];
            let (to, event) = if let Some(colon) = right.find(':') {
                (
                    right[..colon].trim().to_string(),
                    Some(right[colon + 1..].trim().to_string()),
                )
            } else {
                (right.trim().to_string(), None)
            };

            if from != "[*]" && !states.contains(&from) {
                states.push(from.clone());
            }
            if to != "[*]" && !states.contains(&to) {
                states.push(to.clone());
            }

            transitions.push((from, to, event));
        }
    }

    let mut out = Vec::new();
    out.push("  (●) Start".to_string());
    out.push("   │".to_string());
    out.push("   ▼".to_string());

    for (i, state) in states.iter().enumerate() {
        let inner = state.len() + 4;
        out.push(format!(" ┌{}┐", "─".repeat(inner)));
        out.push(format!(" │  {}  │", state));
        out.push(format!(" └{}┘", "─".repeat(inner)));

        if i < states.len() - 1 {
            let next_state = &states[i + 1];
            let event = transitions
                .iter()
                .find(|(f, t, _)| f == state && t == next_state)
                .and_then(|(_, _, e)| e.as_ref())
                .map(|e| format!(" [{}]", e))
                .unwrap_or_default();

            out.push(format!("   │{}", event));
            out.push("   ▼".to_string());
        }
    }

    out.push("   │".to_string());
    out.push("   ▼".to_string());
    out.push("  (◉) End".to_string());

    let result = out.join("\n");
    let line_width = result.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    if max_width > 0 && line_width > max_width {
        return Err(MermaidError::ExceedsMaxWidth(line_width, max_width));
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_flowchart_td() {
        let src = r#"
flowchart TD
    Client["Client App"] -->|HTTPS| Gateway["API Gateway"]
    Gateway --> Service["Auth Service"]
"#;
        let diagram = render(src, 120).unwrap();
        assert!(diagram.contains("Client App"));
        assert!(diagram.contains("API Gateway"));
        assert!(diagram.contains("Auth Service"));
        assert!(diagram.contains("▼"));
    }

    #[test]
    fn test_render_flowchart_lr() {
        let src = r#"
flowchart LR
    A[Browser] --> B[Server]
    B --> C[(Database)]
"#;
        let diagram = render(src, 120).unwrap();
        assert!(diagram.contains("Browser"));
        assert!(diagram.contains("Server"));
        assert!(diagram.contains("Database"));
        assert!(diagram.contains("▶"));
    }

    #[test]
    fn test_render_sequence() {
        let src = r#"
sequenceDiagram
    participant User
    participant Auth as Auth Server
    User->>Auth: Login Request
    Auth-->>User: JWT Token
"#;
        let diagram = render(src, 160).unwrap();
        assert!(diagram.contains("User"));
        assert!(diagram.contains("Auth Server"));
        assert!(diagram.contains("Login Request"));
        assert!(diagram.contains("JWT Token"));
    }

    #[test]
    fn test_render_state_diagram() {
        let src = r#"
stateDiagram-v2
    [*] --> Idle
    Idle --> Processing: job_received
    Processing --> Complete: success
    Complete --> [*]
"#;
        let diagram = render(src, 100).unwrap();
        assert!(diagram.contains("Start"));
        assert!(diagram.contains("Idle"));
        assert!(diagram.contains("Processing"));
        assert!(diagram.contains("Complete"));
        assert!(diagram.contains("End"));
    }
}
