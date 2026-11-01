//! Interactive user question and requirement elicitation tool.
//!
//! Provides the `question` tool: asks one or more structured multiple-choice questions,
//! allowing users to choose from radio options or checkboxes rather than typing free-form answers.

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, IsTerminal, Write};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionItem {
    /// The prompt question text.
    pub question: String,
    /// Candidate options to choose from (put recommended option first).
    pub options: Vec<String>,
    /// Whether the user can select multiple options (checkboxes) instead of one (radio).
    #[serde(default)]
    pub is_multi_select: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionArgs {
    /// List of multiple-choice questions to ask the user.
    pub questions: Vec<QuestionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionAnswer {
    pub question: String,
    pub selected_options: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionResult {
    pub answers: Vec<QuestionAnswer>,
    pub summary: String,
}

/// Asynchronous callback handler for GUI or TUI prompt modals.
pub type QuestionModalHandler = Arc<
    dyn Fn(Vec<QuestionItem>) -> tokio::sync::oneshot::Receiver<Vec<QuestionAnswer>> + Send + Sync,
>;

static QUESTION_HANDLER: Mutex<Option<QuestionModalHandler>> = Mutex::const_new(None);

/// Registers a custom modal handler for TUI / GUI environments.
pub async fn register_question_modal_handler(handler: QuestionModalHandler) {
    let mut lock = QUESTION_HANDLER.lock().await;
    *lock = Some(handler);
}

/// Executes the question elicitation process.
pub async fn execute_question(args: &QuestionArgs) -> Result<QuestionResult> {
    if args.questions.is_empty() {
        return Err(anyhow!(
            "No questions were provided in the 'questions' array."
        ));
    }

    // Check if an interactive TUI modal handler is registered
    let modal_handler = {
        let lock = QUESTION_HANDLER.lock().await;
        lock.clone()
    };

    if let Some(handler) = modal_handler {
        let rx = handler(args.questions.clone());
        let answers = rx
            .await
            .map_err(|_| anyhow!("User cancelled question dialog"))?;
        let summary = format_answers_summary(&answers);
        return Ok(QuestionResult { answers, summary });
    }

    // Headless / non-interactive CI check
    let is_ci = std::env::var("CI").is_ok()
        || std::env::var("NON_INTERACTIVE").is_ok()
        || !std::io::stdin().is_terminal();

    if is_ci {
        // Automatically default to the first option (recommended) for each question
        let mut answers = Vec::with_capacity(args.questions.len());
        for q in &args.questions {
            let selected = if !q.options.is_empty() {
                vec![q.options[0].clone()]
            } else {
                vec!["(No options provided)".to_string()]
            };
            answers.push(QuestionAnswer {
                question: q.question.clone(),
                selected_options: selected,
            });
        }
        let summary = format_answers_summary(&answers);
        return Ok(QuestionResult { answers, summary });
    }

    // Interactive CLI terminal fallback (standard stdin/stdout)
    let mut stdout = io::stdout();
    let stdin = io::stdin();
    let mut reader = stdin.lock();

    let mut answers = Vec::with_capacity(args.questions.len());

    for (q_idx, item) in args.questions.iter().enumerate() {
        writeln!(
            stdout,
            "\n📋 [Question {}/{}] {}",
            q_idx + 1,
            args.questions.len(),
            item.question
        )?;
        for (opt_idx, opt) in item.options.iter().enumerate() {
            writeln!(stdout, "   [{}] {}", opt_idx + 1, opt)?;
        }

        if item.is_multi_select {
            write!(
                stdout,
                "Select one or more options separated by commas (e.g. 1, 3) [default 1]: "
            )?;
        } else {
            write!(stdout, "Select an option number [default 1]: ")?;
        }
        stdout.flush()?;

        let mut input = String::new();
        reader.read_line(&mut input)?;
        let trimmed = input.trim();

        let selected = if trimmed.is_empty() {
            if !item.options.is_empty() {
                vec![item.options[0].clone()]
            } else {
                vec![]
            }
        } else if item.is_multi_select {
            let mut chosen = Vec::new();
            for part in trimmed.split(',') {
                if let Ok(num) = part.trim().parse::<usize>()
                    && num >= 1
                    && num <= item.options.len()
                {
                    chosen.push(item.options[num - 1].clone());
                }
            }
            if chosen.is_empty() && !item.options.is_empty() {
                chosen.push(item.options[0].clone());
            }
            chosen
        } else if let Ok(num) = trimmed.parse::<usize>() {
            if num >= 1 && num <= item.options.len() {
                vec![item.options[num - 1].clone()]
            } else if !item.options.is_empty() {
                vec![item.options[0].clone()]
            } else {
                vec![]
            }
        } else {
            // Free-form fallback
            vec![trimmed.to_string()]
        };

        answers.push(QuestionAnswer {
            question: item.question.clone(),
            selected_options: selected,
        });
    }

    let summary = format_answers_summary(&answers);
    Ok(QuestionResult { answers, summary })
}

fn format_answers_summary(answers: &[QuestionAnswer]) -> String {
    let mut lines = Vec::new();
    for a in answers {
        lines.push(format!(
            "Question: \"{}\" -> Answer(s): {}",
            a.question,
            a.selected_options.join(", ")
        ));
    }
    lines.join("\n")
}
