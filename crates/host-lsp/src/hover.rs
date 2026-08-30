use serde_json::{Value, json};

use handl_kit_canonical::Kit;
use handl_plugin_lexer::TokenKind;
use handl_plugin_token_bool::BoolKind;
use handl_plugin_token_comment::CommentKind;
use handl_plugin_token_delimiter::DelimiterKind;
use handl_plugin_token_doc_comment::DocCommentKind;
use handl_plugin_token_identifier::IdentKind;
use handl_plugin_token_kind::TemplateKind;
use handl_plugin_token_keyword::KeywordKind;
use handl_plugin_token_keyword_as::AsKind;
use handl_plugin_token_keyword_handle::HandleKind;
use handl_plugin_token_null::NullKind;
use handl_plugin_token_number::NumberKind;
use handl_plugin_token_operator::OperatorKind;
use handl_plugin_token_string::StringKind;

use crate::diagnostics::offset_to_position;

pub fn hover_at(source: &str, offset: usize) -> Option<Value> {
    let tokens = Kit::new().lex(source).ok()?;
    let token = tokens
        .iter()
        .find(|t| t.span.start <= offset && offset < t.span.end && !t.kind.is_eof())?;

    let markdown = if let Some(NumberKind::Number(n)) = token.kind.downcast_ref::<NumberKind>() {
        format!("`number`\n\nValue: `{n}`")
    } else if let Some(StringKind::String(s)) = token.kind.downcast_ref::<StringKind>() {
        format!("`string`\n\nValue: `{}`", escape_string(s))
    } else if let Some(BoolKind::Bool(b)) = token.kind.downcast_ref::<BoolKind>() {
        format!("`boolean`\n\nValue: `{b}`")
    } else if token.kind.downcast_ref::<NullKind>().is_some() {
        "`null`".to_string()
    } else if let Some(IdentKind::Ident(name)) = token.kind.downcast_ref::<IdentKind>() {
        format!("`identifier`\n\nName: `{name}`")
    } else if token.kind.downcast_ref::<KeywordKind>().is_some()
        || token.kind.downcast_ref::<AsKind>().is_some()
        || token.kind.downcast_ref::<HandleKind>().is_some()
    {
        format!("`keyword`\n\n`{}`", token.kind)
    } else if token.kind.downcast_ref::<OperatorKind>().is_some() {
        format!("`operator`\n\n`{}`", token.kind)
    } else if token.kind.downcast_ref::<DelimiterKind>().is_some() {
        format!("`punctuation`\n\n`{}`", token.kind)
    } else if token.kind.downcast_ref::<CommentKind>().is_some()
        || token.kind.downcast_ref::<DocCommentKind>().is_some()
    {
        "`comment`".to_string()
    } else if token.kind.downcast_ref::<TemplateKind>().is_some() {
        "`template string`".to_string()
    } else if token.kind.is_eof() {
        return None;
    } else {
        return None;
    };

    let (start_line, start_char) = offset_to_position(source, token.span.start);
    let (end_line, end_char) = offset_to_position(source, token.span.end);

    Some(json!({
        "contents": { "kind": "markdown", "value": markdown },
        "range": {
            "start": { "line": start_line, "character": start_char },
            "end": { "line": end_line, "character": end_char },
        }
    }))
}

fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
