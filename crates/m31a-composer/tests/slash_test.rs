//! Dedicated integration tests for `m31a-composer` (src/slash.rs).
//!
//! Validates:
//! - Canonical slash command catalog completeness and lookup
//! - Alias resolution (e.g. `/?` to `/help`, `/config` to `/settings`)
//! - Quote-aware tokenizer handling of single/double quotes and escapes
//! - Slash command parsing and resolution into structured command calls
//! - Autocomplete candidate filtering and SlashPopup navigation

use m31a_composer::slash::{
    CANONICAL_SLASH_COMMANDS, CommandExecutionClass, ContextAvailability, SlashPopup, find_command,
    get_slash_completions, matching_commands, parse_and_resolve, tokenize_slash_command,
};

#[test]
fn test_canonical_catalog_and_lookup() {
    assert!(!CANONICAL_SLASH_COMMANDS.is_empty());

    let help_cmd = find_command("/help").expect("Find /help");
    assert_eq!(help_cmd.name, "/help");
    assert_eq!(help_cmd.execution_class, CommandExecutionClass::ClientLocal);
    assert_eq!(help_cmd.availability, ContextAvailability::Always);

    // Alias lookup
    let alias_help = find_command("/?").expect("Find alias /?");
    assert_eq!(alias_help.name, "/help");

    let alias_settings = find_command("/config").expect("Find alias /config");
    assert_eq!(alias_settings.name, "/settings");
}

#[test]
fn test_matching_commands_prefix_search() {
    let se_matches = matching_commands("/se");
    assert!(se_matches.iter().any(|c| c.name == "/settings"));
    assert!(se_matches.iter().any(|c| c.name == "/setup"));

    let empty_matches = matching_commands("/nonexistent_prefix_xyz");
    assert!(empty_matches.is_empty());
}

#[test]
fn test_tokenize_slash_command_quotes_and_escapes() {
    let line1 = "/test arg1 \"arg with spaces\" 'single quoted'";
    let tokens = tokenize_slash_command(line1);
    assert_eq!(
        tokens,
        vec!["/test", "arg1", "arg with spaces", "single quoted"]
    );

    let line2 = "/run \"quoted with \\\"nested\\\" quotes\"";
    let tokens2 = tokenize_slash_command(line2);
    assert_eq!(tokens2.len(), 2);
    assert_eq!(tokens2[0], "/run");
    assert!(tokens2[1].contains("nested"));
}

#[test]
fn test_parse_and_resolve_slash_command() {
    let parsed = parse_and_resolve("/config edit").expect("parse config alias");
    assert_eq!(parsed.command.name, "/settings");
    assert_eq!(parsed.args, vec!["edit"]);

    let parsed_setup = parse_and_resolve("/setup wizard").expect("parse setup");
    assert_eq!(parsed_setup.command.name, "/setup");
    assert_eq!(parsed_setup.args, vec!["wizard"]);
}

#[test]
fn test_slash_completions_and_popup() {
    let completions = get_slash_completions("/set");
    assert!(!completions.is_empty());
    assert!(
        completions
            .iter()
            .any(|item| item.name == "/settings" || item.name == "/setup")
    );

    let mut popup = SlashPopup::new();
    popup.update("/set");
    assert!(popup.is_active);

    if popup.matches.len() >= 2 {
        assert_eq!(popup.selected_index, 0);
        popup.move_down();
        assert_eq!(popup.selected_index, 1);
        popup.move_up();
        assert_eq!(popup.selected_index, 0);
    }

    popup.update("");
    assert!(!popup.is_active);
    assert!(popup.matches.is_empty());
}
