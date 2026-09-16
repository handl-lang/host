use crate::document;
use crate::protocol::Message;

pub mod call_hierarchy;
pub mod code_actions;
pub mod code_lens;
pub mod color;
pub mod completion;
pub mod definition;
pub mod diagnostics;
pub mod document_highlight;
pub mod document_link;
pub mod document_symbols;
pub mod execute_command;
pub mod folding_ranges;
pub mod formatting;
pub mod hover;
pub mod initialize;
pub mod inlay_hints;
pub mod lifecycle;
pub mod linked_editing;
pub mod moniker;
pub mod references;
pub mod rename;
pub mod selection_ranges;
pub mod semantic_tokens;
pub mod signature_help;
pub mod type_hierarchy;
pub mod workspace;

pub(crate) fn position_offset_from_msg(msg: &Message, source: &str) -> usize {
    let (line, character) = position_line_char(msg);
    document::position_to_offset(source, line, character)
}

/// Extract the (line, character) position from a message's `position` field.
pub(crate) fn position_line_char(msg: &Message) -> (u32, u32) {
    msg.params
        .as_ref()
        .and_then(|p| p.get("position"))
        .and_then(|pos| {
            let line = pos.get("line").and_then(|l| l.as_u64()).unwrap_or(0) as u32;
            let character = pos.get("character").and_then(|c| c.as_u64()).unwrap_or(0) as u32;
            Some((line, character))
        })
        .unwrap_or((0, 0))
}

pub use self::initialize::handle_initialize;
pub use self::semantic_tokens::handle_semantic_tokens_full;
pub use self::semantic_tokens::handle_semantic_tokens_range;
pub use self::lifecycle::handle_shutdown;
pub use self::hover::handle_hover;
pub use self::lifecycle::handle_unknown;
pub use self::definition::handle_type_definition;
pub use self::definition::handle_implementation;
pub use self::linked_editing::handle_linked_editing;
pub use self::moniker::handle_moniker;
pub use self::completion::handle_completion_resolve;
pub use self::inlay_hints::handle_inlay_hint_resolve;
pub use self::execute_command::handle_execute_command;
pub use self::document_link::handle_document_link;
pub use self::call_hierarchy::handle_prepare_call_hierarchy;
pub use self::call_hierarchy::handle_call_hierarchy_incoming;
pub use self::call_hierarchy::handle_call_hierarchy_outgoing;
pub use self::type_hierarchy::handle_prepare_type_hierarchy;
pub use self::type_hierarchy::handle_type_hierarchy_supertypes;
pub use self::type_hierarchy::handle_type_hierarchy_subtypes;
pub use self::color::handle_document_color;
pub use self::color::handle_color_presentation;
#[cfg(test)]
pub use self::lifecycle::handle_log;
pub use self::completion::handle_completion;
pub use self::signature_help::handle_signature_help;
pub use self::document_symbols::handle_document_symbols;
pub use self::formatting::handle_formatting;
pub use self::formatting::handle_on_type_formatting;
pub use self::formatting::handle_range_formatting;
pub use self::folding_ranges::handle_folding_ranges;
pub use self::document_highlight::handle_document_highlight;
pub use self::selection_ranges::handle_selection_ranges;
pub use self::code_actions::handle_code_actions;
pub use self::workspace::handle_workspace_symbols;
pub use self::inlay_hints::handle_inlay_hints;
pub use self::rename::handle_prepare_rename;
pub use self::rename::handle_rename;
pub use self::definition::handle_definition;
pub use self::references::handle_references;
pub use self::diagnostics::handle_publish_diagnostics;
pub use self::code_lens::handle_code_lens;
#[cfg(test)]
pub use self::initialize::build_initialize_result;
