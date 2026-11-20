//! Semantic data models for native OS accessibility trees, elements, and actions.

use serde::{Deserialize, Serialize};

/// Bounding rectangle for windows and accessible elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    /// Returns the center point (x, y) of the rectangle.
    pub fn center(&self) -> (i32, i32) {
        (
            self.x + (self.width / 2) as i32,
            self.y + (self.height / 2) as i32,
        )
    }
}

/// Native desktop application summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppInfo {
    pub id: String,
    pub name: String,
    pub pid: u32,
    pub is_active: bool,
    pub windows: Vec<WindowInfo>,
}

/// Window summary inside an application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowInfo {
    pub id: String,
    pub title: String,
    pub bounds: Rect,
    pub is_focused: bool,
}

/// Semantic role of an accessible element.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementRole {
    Window,
    Dialog,
    Panel,
    Button,
    TextField,
    TextArea,
    CheckBox,
    RadioButton,
    ComboBox,
    Menu,
    MenuItem,
    Tab,
    TabList,
    Table,
    Row,
    Cell,
    List,
    ListItem,
    Text,
    Heading,
    Link,
    Image,
    Slider,
    ScrollBar,
    Custom(String),
}

/// A node in the hierarchical accessibility tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessibleElement {
    /// Zero-based indexed position in the depth-first traversal of the active tree.
    pub index: usize,
    /// Semantic UI role.
    pub role: ElementRole,
    /// Element name, accessible label, or text.
    pub name: String,
    /// Current input or displayed value (e.g. text in textfield, checkbox state).
    pub value: Option<String>,
    /// Screen coordinates of the element if rendered.
    pub bounds: Option<Rect>,
    /// State flags (e.g. "focused", "disabled", "selected", "checked").
    pub states: Vec<String>,
    /// Actions exposed by accessibility protocol (e.g. "press", "activate", "select").
    pub actions: Vec<String>,
    /// Child accessible elements.
    pub children: Vec<AccessibleElement>,
}

impl AccessibleElement {
    /// Recursively counts all elements in this subtree.
    pub fn total_nodes(&self) -> usize {
        1 + self.children.iter().map(|c| c.total_nodes()).sum::<usize>()
    }

    /// Finds an element by its index in the tree.
    pub fn find_by_index(&self, target_index: usize) -> Option<&AccessibleElement> {
        if self.index == target_index {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find_by_index(target_index) {
                return Some(found);
            }
        }
        None
    }

    /// Finds a mutable reference to an element by its index.
    pub fn find_by_index_mut(&mut self, target_index: usize) -> Option<&mut AccessibleElement> {
        if self.index == target_index {
            return Some(self);
        }
        for child in &mut self.children {
            if let Some(found) = child.find_by_index_mut(target_index) {
                return Some(found);
            }
        }
        None
    }
}

/// Complete hierarchical snapshot of an application's accessibility tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppState {
    pub state_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub app_id: String,
    pub app_name: String,
    pub window_title: String,
    pub root: AccessibleElement,
    pub element_count: usize,
}

/// Semantic action to perform on an accessible element.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum ElementAction {
    Click,
    DoubleClick,
    RightClick,
    SetValue(String),
    Select,
    Toggle,
    PerformAction(String),
}

/// Coordinate-based mouse fallback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MouseAction {
    Click { button: String },
    DoubleClick { button: String },
    Move,
    Drag { to_x: i32, to_y: i32 },
    Scroll { dx: i32, dy: i32 },
}

/// Outcome of a CUA operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionResult {
    pub success: bool,
    pub message: String,
    pub new_state_id: Option<String>,
    pub action_sent: bool,
}
