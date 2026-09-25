//! Deterministic MiniJinja template rendering under strict byte and parameter bounds.

use crate::prompt::contract::{MAX_RENDERED_BYTES, PromptContract};
use crate::prompt::error::PromptError;
use crate::prompt::parameter::MAX_PARAMETER_BYTES;
use minijinja::Environment;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

/// Outcome of rendering a prompt contract with validated parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderedPrompt {
    /// Source prompt contract ID.
    pub prompt_id: String,
    /// Source prompt contract version.
    pub prompt_version: u32,
    /// Final rendered prompt string.
    pub rendered_text: String,
    /// Source prompt content hash.
    pub content_hash: String,
    /// Names of parameters that were supplied.
    pub supplied_parameters: Vec<String>,
}

/// Deterministically render a PromptContract using MiniJinja without filesystem or external side effects.
pub fn render_prompt(
    contract: &PromptContract,
    parameters: &BTreeMap<String, String>,
    strict_parameters: bool,
) -> Result<RenderedPrompt, PromptError> {
    // 1. Check parameter size bounds to prevent memory exhaustion
    let total_param_bytes: usize = parameters.iter().map(|(k, v)| k.len() + v.len()).sum();
    if total_param_bytes > MAX_PARAMETER_BYTES {
        return Err(PromptError::PromptRenderFailure {
            prompt_id: contract.id.clone(),
            reason: format!(
                "parameters byte size ({}) exceeds maximum permitted bound ({})",
                total_param_bytes, MAX_PARAMETER_BYTES
            ),
        });
    }

    // 2. Validate declared parameters
    let mut render_context: BTreeMap<String, String> = BTreeMap::new();
    let declared_names: HashSet<&str> = contract
        .input_parameters
        .iter()
        .map(|p| p.name.as_str())
        .collect();

    for param in &contract.input_parameters {
        if let Some(val) = parameters.get(&param.name) {
            render_context.insert(param.name.clone(), val.clone());
        } else if let Some(def) = &param.default_value {
            render_context.insert(param.name.clone(), def.clone());
        } else if param.is_required {
            return Err(PromptError::PromptParameterMissing {
                prompt_id: contract.id.clone(),
                parameter: param.name.clone(),
            });
        }
    }

    // 3. If strict mode enabled, reject unknown parameters
    if strict_parameters {
        for key in parameters.keys() {
            if !declared_names.contains(key.as_str()) {
                return Err(PromptError::PromptParameterInvalid {
                    prompt_id: contract.id.clone(),
                    parameter: key.clone(),
                    reason: "unknown parameter not declared in prompt contract".to_string(),
                });
            }
        }
    }

    // 4. Render using MiniJinja without external loaders
    let mut env = Environment::new();
    env.add_template(&contract.id, &contract.template_body)
        .map_err(|e| PromptError::PromptRenderFailure {
            prompt_id: contract.id.clone(),
            reason: format!("failed to compile template: {}", e),
        })?;

    let tmpl = env
        .get_template(&contract.id)
        .map_err(|e| PromptError::PromptRenderFailure {
            prompt_id: contract.id.clone(),
            reason: format!("failed to retrieve compiled template: {}", e),
        })?;

    let rendered = tmpl
        .render(&render_context)
        .map_err(|e| PromptError::PromptRenderFailure {
            prompt_id: contract.id.clone(),
            reason: format!("rendering evaluation failed: {}", e),
        })?;

    if rendered.len() > MAX_RENDERED_BYTES {
        return Err(PromptError::PromptRenderFailure {
            prompt_id: contract.id.clone(),
            reason: format!(
                "rendered output size ({}) exceeds maximum limit ({})",
                rendered.len(),
                MAX_RENDERED_BYTES
            ),
        });
    }

    Ok(RenderedPrompt {
        prompt_id: contract.id.clone(),
        prompt_version: contract.version,
        rendered_text: rendered,
        content_hash: contract.content_hash.clone(),
        supplied_parameters: render_context.into_keys().collect(),
    })
}
