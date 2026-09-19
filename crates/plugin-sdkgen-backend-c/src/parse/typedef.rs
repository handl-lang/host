//! Typedef declarations → `Typedef`.

use clang_sys::*;
use handl_plugin_sdkgen::Typedef;

use crate::cx::{cursor_name, doc_comment};
use crate::map::map_arg_type;
use crate::parse::ParseCtx;

pub(crate) fn parse_typedef(cursor: CXCursor, ctx: &ParseCtx) -> Result<Typedef, String> {
    let name = cursor_name(cursor);
    if name.is_empty() {
        return Err("anonymous typedef is not supported".into());
    }
    let doc = doc_comment(cursor);
    let underlying = unsafe { clang_getTypedefDeclUnderlyingType(cursor) };
    let (port, descriptor) = map_arg_type(underlying, ctx.pointer_width)?;
    Ok(Typedef {
        name,
        port,
        descriptor,
        doc,
    })
}
