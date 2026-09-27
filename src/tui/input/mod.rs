//! Reusable Input & Form Engine (COP-01, COP-02, CFX-03, D-04).

pub mod action;
pub mod form;
pub mod text_input;

pub use action::{UiAction, normalize_key_event};
pub use form::{Form, FormField, ValidationRule};
pub use text_input::{InputMasking, InputMode, TextInput};
