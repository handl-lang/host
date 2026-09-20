//! Lightweight WebIDL grammar for the SDK generator.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token<'a> {
    Ident(&'a str),
    Number(&'a str),
    String(&'a str),
    Sym(char),
    /// `//` or `/* */` comment inner text (markers stripped). `same_line`
    /// marks comments that begin on the same line as the preceding
    /// token — those document the item they trail rather than the next.
    /// `blank_after` marks comments followed by a blank line: a block
    /// ending that way is file provenance (e.g. webref headers), not a
    /// doc comment.
    Comment {
        text: &'a str,
        same_line: bool,
        blank_after: bool,
    },
    Eof,
}

struct Tokenizer<'a> {
    src: &'a str,
    pos: usize,
    /// A newline was crossed since the last emitted token — feeds the
    /// `same_line` classification on [`Token::Comment`].
    newline: bool,
}

impl<'a> Tokenizer<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src,
            pos: 0,
            // The start of the file counts as a fresh line — a comment
            // before any token is a leading comment, not a trailing one.
            newline: true,
        }
    }

    fn eof(&self) -> bool {
        self.pos >= self.src.len()
    }

    fn peek_char(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn advance_char(&mut self) -> Option<char> {
        let c = self.peek_char()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    /// Skip whitespace only, tracking whether a newline was crossed so
    /// the next [`Token::Comment`] can be classified `same_line`.
    fn skip_ws(&mut self) {
        while let Some(c) = self.peek_char() {
            if !c.is_whitespace() {
                break;
            }
            if c == '\n' {
                self.newline = true;
            }
            self.advance_char();
        }
    }

    fn next_token(&mut self) -> Token<'a> {
        self.skip_ws();
        if self.eof() {
            return Token::Eof;
        }
        let rest = &self.src[self.pos..];
        if let Some(inner) = rest.strip_prefix("//") {
            let same_line = !self.newline;
            let end = inner.find('\n').unwrap_or(inner.len());
            // `///`/`//!`-style comments carry an extra marker char.
            let text = inner[..end].strip_prefix('/').unwrap_or(&inner[..end]);
            let after = self.pos + 2 + end;
            let blank_after = blanks_ahead(self.src, after);
            self.pos = after;
            return Token::Comment {
                text,
                same_line,
                blank_after,
            };
        }
        if let Some(inner) = rest.strip_prefix("/*") {
            let same_line = !self.newline;
            let (text, consumed) = match inner.find("*/") {
                Some(n) => (&inner[..n], 2 + n + 2),
                None => (inner, rest.len()),
            };
            let after = self.pos + consumed;
            let blank_after = blanks_ahead(self.src, after);
            self.pos = after;
            return Token::Comment {
                text,
                same_line,
                blank_after,
            };
        }
        self.newline = false;
        let start = self.pos;
        let c = self.peek_char().unwrap();
        match c {
            '"' => {
                self.advance_char(); // skip opening quote
                let value_start = self.pos;
                while let Some(c2) = self.peek_char() {
                    if c2 == '"' {
                        let value = &self.src[value_start..self.pos];
                        self.advance_char(); // closing quote
                        return Token::String(value);
                    }
                    self.advance_char();
                }
                return Token::String(&self.src[value_start..self.pos]);
            }
            _ if c.is_ascii_digit() => {
                if c == '0' && matches!(self.src.as_bytes().get(self.pos + 1), Some(b'x' | b'X')) {
                    self.advance_char();
                    self.advance_char();
                    while let Some(c2) = self.peek_char() {
                        if c2.is_ascii_hexdigit() {
                            self.advance_char();
                        } else {
                            break;
                        }
                    }
                } else {
                    while let Some(c2) = self.peek_char() {
                        if c2.is_ascii_digit()
                            || c2 == '.'
                            || c2 == 'e'
                            || c2 == 'E'
                            || c2 == '+'
                            || c2 == '-'
                        {
                            self.advance_char();
                        } else {
                            break;
                        }
                    }
                }
                Token::Number(&self.src[start..self.pos])
            }
            _ if c.is_ascii_alphabetic() || c == '_' => {
                while let Some(c2) = self.peek_char() {
                    if c2.is_ascii_alphanumeric() || c2 == '_' {
                        self.advance_char();
                    } else {
                        break;
                    }
                }
                Token::Ident(&self.src[start..self.pos])
            }
            _ => {
                self.advance_char();
                Token::Sym(c)
            }
        }
    }

    fn tokenize(mut self) -> Vec<Token<'a>> {
        let mut tokens = Vec::new();
        loop {
            let t = self.next_token();
            if t == Token::Eof {
                break;
            }
            tokens.push(t);
        }
        tokens
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Void,
    Undefined,
    Bool,
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
    String,
    Bytes,
    Any,
    Object,
    Promise(Box<Type>),
    Sequence(Box<Type>),
    Record(Box<Type>, Box<Type>),
    Nullable(Box<Type>),
    Union(Vec<Type>),
    Handle(String),
}

impl Type {
    pub fn from_name(name: &str) -> Self {
        match name {
            "void" => Type::Void,
            "undefined" => Type::Undefined,
            "boolean" => Type::Bool,
            "byte" => Type::I8,
            "octet" => Type::U8,
            "short" => Type::I16,
            "unsigned short" => Type::U16,
            "long" => Type::I32,
            "unsigned long" => Type::U32,
            "long long" => Type::I64,
            "unsigned long long" => Type::U64,
            "float" | "unrestricted float" => Type::F32,
            "double" | "unrestricted double" => Type::F64,
            "DOMString" | "USVString" | "ByteString" => Type::String,
            "ArrayBuffer" | "ArrayBufferView" | "BufferSource" | "Int8Array" | "Uint8Array"
            | "Int16Array" | "Uint16Array" | "Int32Array" | "Uint32Array" | "Float32Array"
            | "Float64Array" | "Uint8ClampedArray" | "SharedArrayBuffer" => Type::Bytes,
            "any" => Type::Any,
            "object" => Type::Object,
            "Promise" => Type::Promise(Box::new(Type::Void)),
            "sequence" => Type::Sequence(Box::new(Type::Any)),
            "record" => Type::Record(Box::new(Type::Any), Box::new(Type::Any)),
            "FrozenArray" | "ObservableArray" => Type::Sequence(Box::new(Type::Any)),
            _ => Type::Handle(name.into()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Argument {
    pub name: String,
    pub ty: Type,
    pub optional: bool,
    pub variadic: bool,
}

#[derive(Debug, Clone)]
pub enum Member {
    Const {
        name: String,
        ty: Type,
        value: i64,
        doc: Option<String>,
    },
    Attribute {
        name: String,
        ty: Type,
        readonly: bool,
        doc: Option<String>,
    },
    DictionaryField {
        name: String,
        ty: Type,
        required: bool,
        doc: Option<String>,
    },
    Operation {
        name: String,
        return_ty: Type,
        args: Vec<Argument>,
        doc: Option<String>,
    },
    Constructor {
        args: Vec<Argument>,
        doc: Option<String>,
    },
    Skip(String),
}

/// One enum value plus its optional doc comment.
#[derive(Debug, Clone)]
pub struct EnumValue {
    pub name: String,
    pub doc: Option<String>,
}

/// Doc-comment plumbing shared by [`Member`] and [`AstDef`]: leading
/// comment lines are stashed in [`Parser::pending`] and stamped via
/// `set_doc`; same-line trailing comments are appended via `append_doc`.
trait DocTarget {
    fn doc_mut(&mut self) -> Option<&mut Option<String>>;
    fn set_doc(&mut self, doc: String) {
        if let Some(slot) = self.doc_mut() {
            *slot = Some(doc);
        }
    }
    fn append_doc(&mut self, extra: &str) {
        if let Some(slot) = self.doc_mut() {
            match slot {
                Some(d) => {
                    d.push('\n');
                    d.push_str(extra);
                }
                None => *slot = Some(extra.to_string()),
            }
        }
    }
}

impl DocTarget for Member {
    fn doc_mut(&mut self) -> Option<&mut Option<String>> {
        match self {
            Member::Const { doc, .. }
            | Member::Attribute { doc, .. }
            | Member::DictionaryField { doc, .. }
            | Member::Operation { doc, .. }
            | Member::Constructor { doc, .. } => Some(doc),
            Member::Skip(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum AstDef {
    Interface {
        name: String,
        base: Option<String>,
        members: Vec<Member>,
        partial: bool,
        doc: Option<String>,
    },
    Mixin {
        name: String,
        members: Vec<Member>,
        partial: bool,
        doc: Option<String>,
    },
    Dictionary {
        name: String,
        base: Option<String>,
        members: Vec<Member>,
        partial: bool,
        doc: Option<String>,
    },
    Namespace {
        name: String,
        members: Vec<Member>,
        partial: bool,
        doc: Option<String>,
    },
    Enum {
        name: String,
        values: Vec<EnumValue>,
        doc: Option<String>,
    },
    Typedef {
        name: String,
        ty: Type,
        doc: Option<String>,
    },
    Callback {
        name: String,
        return_ty: Type,
        args: Vec<Argument>,
        partial: bool,
        doc: Option<String>,
    },
    Skip(String),
}

impl DocTarget for AstDef {
    fn doc_mut(&mut self) -> Option<&mut Option<String>> {
        match self {
            AstDef::Interface { doc, .. }
            | AstDef::Mixin { doc, .. }
            | AstDef::Dictionary { doc, .. }
            | AstDef::Namespace { doc, .. }
            | AstDef::Enum { doc, .. }
            | AstDef::Typedef { doc, .. }
            | AstDef::Callback { doc, .. } => Some(doc),
            AstDef::Skip(_) => None,
        }
    }
}

impl AstDef {
    pub fn name(&self) -> &str {
        match self {
            AstDef::Interface { name, .. }
            | AstDef::Mixin { name, .. }
            | AstDef::Dictionary { name, .. }
            | AstDef::Namespace { name, .. }
            | AstDef::Enum { name, .. }
            | AstDef::Typedef { name, .. }
            | AstDef::Callback { name, .. } => name,
            AstDef::Skip(_) => "",
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            AstDef::Interface { .. } => "interface",
            AstDef::Mixin { .. } => "mixin",
            AstDef::Dictionary { .. } => "dictionary",
            AstDef::Namespace { .. } => "namespace",
            AstDef::Enum { .. } => "enum",
            AstDef::Typedef { .. } => "typedef",
            AstDef::Callback { .. } => "callback",
            AstDef::Skip(_) => "skip",
        }
    }

    /// The doc comment attached to this definition, if any.
    pub fn doc(&self) -> Option<&str> {
        match self {
            AstDef::Interface { doc, .. }
            | AstDef::Mixin { doc, .. }
            | AstDef::Dictionary { doc, .. }
            | AstDef::Namespace { doc, .. }
            | AstDef::Enum { doc, .. }
            | AstDef::Typedef { doc, .. }
            | AstDef::Callback { doc, .. } => doc.as_deref(),
            AstDef::Skip(_) => None,
        }
    }

    pub fn members_mut(&mut self) -> Option<&mut Vec<Member>> {
        match self {
            Self::Interface { members, .. }
            | Self::Mixin { members, .. }
            | Self::Dictionary { members, .. }
            | Self::Namespace { members, .. } => Some(members),
            _ => None,
        }
    }

    /// Update `self` in place — counter decrement is a small mutation that the
    /// match guard already guards; `...` is used to keep the arm short.

    pub fn set_partial(&mut self, partial: bool) {
        match self {
            Self::Interface { partial: p, .. }
            | Self::Mixin { partial: p, .. }
            | Self::Dictionary { partial: p, .. }
            | Self::Namespace { partial: p, .. }
            | Self::Callback { partial: p, .. } => *p = partial,
            _ => {}
        }
    }

    /// Spawn a single member — actually parses the next chunk as one and pushes
    /// the result onto `self.members`. The helper avoids repeating the bracket
    /// count twice per loop iteration.

    pub fn merge(&mut self, mut other: AstDef, diags: &mut Vec<String>) {
        if self.kind() != other.kind() {
            diags.push(format!(
                "cannot merge partial {} into {}",
                other.kind(),
                self.kind()
            ));
            return;
        }
        // A doc on the non-partial declaration wins; a doc that only
        // exists on the partial still lands on the merged def.
        let other_doc = other.doc_mut().and_then(|d| d.take());
        if let (Some(slot), Some(d)) = (self.doc_mut(), other_doc)
            && slot.is_none()
        {
            *slot = Some(d);
        }
        if let (Some(a), Some(b)) = (self.members_mut(), other.members_mut()) {
            a.extend(std::mem::take(b));
        }
        if let (
            AstDef::Callback {
                return_ty: a, args, ..
            },
            AstDef::Callback {
                return_ty: b,
                args: bargs,
                ..
            },
        ) = (self, other)
        {
            if *a == Type::Void {
                *a = b;
                if bargs.len() > args.len() {
                    *args = bargs;
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Container {
    Interface,
    Mixin,
    Dictionary,
    Namespace,
}

struct Parser<'a> {
    tokens: Vec<Token<'a>>,
    pos: usize,
    diags: Vec<String>,
    /// Leading doc lines collected from own-line comments, consumed by
    /// the next def/member/enum-value parse via [`Parser::take_pending`].
    pending: Vec<String>,
}

/// Comment inner text → doc lines: trims each line and drops `*`
/// margins from `/* */` blocks.
fn normalize_comment(raw: &str) -> Vec<String> {
    raw.lines()
        .map(|l| l.trim().trim_start_matches('*').trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Whether a blank line (two or more consecutive newlines, other
/// whitespace allowed mid-run) separates `pos` from the next non-whitespace
/// character.
fn blanks_ahead(src: &str, pos: usize) -> bool {
    let mut newlines = 0;
    for c in src[pos..].chars() {
        if !c.is_whitespace() {
            break;
        }
        if c == '\n' {
            newlines += 1;
        }
    }
    newlines >= 2
}

impl<'a> Parser<'a> {
    fn new(tokens: Vec<Token<'a>>) -> Self {
        Self {
            tokens,
            pos: 0,
            diags: Vec::new(),
            pending: Vec::new(),
        }
    }

    fn peek(&self) -> &Token<'a> {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn at(&self, offset: usize) -> &Token<'a> {
        self.tokens.get(self.pos + offset).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) {
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }

    fn diag(&mut self, msg: &str) {
        self.diags.push(msg.to_string());
    }

    /// Consume a run of comment tokens. Own-line comments accumulate in
    /// `pending` (docs for the next item); same-line comments are
    /// returned so the caller can append them to the item just parsed.
    /// A comment block whose last comment is followed by a blank line is
    /// file provenance, not documentation — dropped entirely.
    fn drain_comments(&mut self) -> Option<String> {
        let mut trailing = Vec::new();
        let mut block: Vec<String> = Vec::new();
        let mut block_blank_terminated = false;
        while let Token::Comment {
            text,
            same_line,
            blank_after,
        } = self.peek()
        {
            let (text, same_line, blank_after) = (*text, *same_line, *blank_after);
            self.advance();
            let lines = normalize_comment(text);
            if same_line {
                trailing.extend(lines);
                continue;
            }
            block.extend(lines);
            block_blank_terminated = blank_after;
        }
        if !block_blank_terminated {
            self.pending.extend(block);
        }
        if trailing.is_empty() {
            None
        } else {
            Some(trailing.join("\n"))
        }
    }

    /// Take the accumulated leading doc lines, if any.
    fn take_pending(&mut self) -> Option<String> {
        if self.pending.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.pending).join("\n"))
        }
    }

    fn match_sym(&mut self, c: char) -> bool {
        if matches!(self.peek(), Token::Sym(c2) if c2 == &c) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn consume_if_ident(&mut self, s: &str) -> bool {
        if let Token::Ident(t) = self.peek() {
            if *t == s {
                self.advance();
                return true;
            }
        }
        false
    }

    fn expect_ident(&mut self, ctx: &str) -> String {
        match self.peek() {
            Token::Ident(s) => {
                let r = (*s).to_string();
                self.advance();
                r
            }
            _ => {
                self.diag(&format!("expected identifier ({ctx})"));
                String::new()
            }
        }
    }

    fn expect_sym(&mut self, c: char) {
        if !self.match_sym(c) {
            self.diag(&format!("expected '{c}'"));
        }
    }

    fn skip_extended_attrs(&mut self) {
        loop {
            // Comments wedged between attrs still count as docs for the
            // item the attrs decorate.
            while let Token::Comment { text, .. } = self.peek() {
                let text = *text;
                self.advance();
                self.pending.extend(normalize_comment(text));
            }
            if !self.match_sym('[') {
                break;
            }
            let mut depth = 1;
            while depth > 0 && !matches!(self.peek(), Token::Eof) {
                if self.match_sym('[') {
                    depth += 1;
                } else if self.match_sym(']') {
                    depth -= 1;
                } else {
                    self.advance();
                }
            }
        }
    }

    fn skip_to_semi(&mut self) {
        let mut depth = 0;
        while !matches!(self.peek(), Token::Eof) {
            match self.peek() {
                Token::Sym('(') | Token::Sym('[') | Token::Sym('{') => depth += 1,
                Token::Sym(')') | Token::Sym(']') | Token::Sym('}') if depth > 0 => depth -= 1,
                Token::Sym(';') if depth == 0 => {
                    self.advance();
                    break;
                }
                Token::Sym('}') if depth == 0 => break,
                _ => {}
            }
            self.advance();
        }
    }

    fn skip_generic_args(&mut self) {
        if !self.match_sym('<') {
            return;
        }
        let mut depth = 1;
        while depth > 0 && !matches!(self.peek(), Token::Eof) {
            if self.match_sym('<') {
                depth += 1;
            } else if self.match_sym('>') {
                depth -= 1;
            } else {
                self.advance();
            }
        }
    }

    fn consume_trailing_semi(&mut self) {
        if self.match_sym(';') {
            return;
        }
        if matches!(self.peek(), Token::Sym('}') | Token::Eof) {
            return;
        }
        self.diag("expected ';'");
        self.skip_to_semi();
    }

    fn parse_type_name(&mut self) -> String {
        let first = self.expect_ident("type name");
        match first.as_str() {
            "unsigned" => {
                let second = self.expect_ident("after 'unsigned'");
                if second == "short" {
                    "unsigned short".into()
                } else if second == "long" {
                    if self.consume_if_ident("long") {
                        "unsigned long long".into()
                    } else {
                        "unsigned long".into()
                    }
                } else {
                    format!("unsigned {second}")
                }
            }
            "long" => {
                if self.consume_if_ident("long") {
                    "long long".into()
                } else {
                    "long".into()
                }
            }
            "unrestricted" => {
                let second = self.expect_ident("after 'unrestricted'");
                if second == "float" {
                    "unrestricted float".into()
                } else if second == "double" {
                    "unrestricted double".into()
                } else {
                    format!("unrestricted {second}")
                }
            }
            _ => first,
        }
    }

    fn parse_base_type(&mut self) -> Type {
        self.skip_extended_attrs();
        if self.match_sym('(') {
            let mut types = vec![self.parse_type()];
            while self.consume_if_ident("or") {
                types.push(self.parse_type());
            }
            self.expect_sym(')');
            if types.len() == 1 {
                types.into_iter().next().unwrap()
            } else {
                Type::Union(types)
            }
        } else {
            let name = self.parse_type_name();
            Type::from_name(&name)
        }
    }

    fn parse_type_suffix(&mut self, mut t: Type) -> Type {
        if self.match_sym('?') {
            t = Type::Nullable(Box::new(t));
        }
        if self.match_sym('<') {
            match t {
                Type::Record(_, _) => {
                    let k = self.parse_type();
                    self.expect_sym(',');
                    let v = self.parse_type();
                    t = Type::Record(Box::new(k), Box::new(v));
                }
                _ => {
                    let inner = self.parse_type();
                    t = match t {
                        Type::Promise(_) => Type::Promise(Box::new(inner)),
                        _ => Type::Sequence(Box::new(inner)),
                    };
                }
            }
            self.expect_sym('>');
        }
        if self.match_sym('[') {
            self.expect_sym(']');
            t = Type::Sequence(Box::new(t));
        }
        t
    }

    pub fn parse_type(&mut self) -> Type {
        let base = self.parse_base_type();
        self.parse_type_suffix(base)
    }

    fn parse_const_value(&mut self) -> i64 {
        match self.peek() {
            Token::Number(s) => {
                let r = match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                    Some(h) => i64::from_str_radix(h, 16).unwrap_or(0),
                    None => s.parse::<i64>().unwrap_or(0),
                };
                self.advance();
                r
            }
            Token::Ident("true") => {
                self.advance();
                1
            }
            Token::Ident("false") => {
                self.advance();
                0
            }
            Token::String(_) => {
                self.advance();
                0
            }
            _ => {
                self.diag("expected constant value");
                0
            }
        }
    }

    fn parse_args_body(&mut self) -> Vec<Argument> {
        let mut args = Vec::new();
        if self.match_sym(')') {
            return args;
        }
        loop {
            self.skip_extended_attrs();
            let optional = self.consume_if_ident("optional");
            let ty = self.parse_type();
            let mut variadic = false;
            if matches!(self.peek(), Token::Sym('.'))
                && matches!(self.at(1), Token::Sym('.'))
                && matches!(self.at(2), Token::Sym('.'))
            {
                self.advance();
                self.advance();
                self.advance();
                variadic = true;
            }
            let name = self.expect_ident("argument name");
            if self.match_sym('=') {
                self.skip_arg_default();
            }
            args.push(Argument {
                name,
                ty,
                optional,
                variadic,
            });
            if self.match_sym(',') {
                continue;
            }
            if self.match_sym(')') {
                break;
            }
            self.diag("expected ',' or ')' in argument list");
            self.skip_to_semi();
            break;
        }
        args
    }

    fn skip_arg_default(&mut self) {
        let mut depth = 0;
        while !matches!(self.peek(), Token::Eof) {
            match self.peek() {
                Token::Sym('(') | Token::Sym('[') | Token::Sym('{') => depth += 1,
                Token::Sym(')') | Token::Sym(']') | Token::Sym('}') if depth > 0 => depth -= 1,
                Token::Sym(',') | Token::Sym(';') if depth == 0 => return,
                Token::Sym(')') if depth == 0 => return,
                _ => {}
            }
            self.advance();
        }
    }

    fn peek_is_attribute_start(&self) -> bool {
        matches!(self.peek(), Token::Ident("readonly"))
            || matches!(self.peek(), Token::Ident("attribute"))
    }

    fn parse_attribute(&mut self) -> Option<Member> {
        let mut readonly = false;
        if self.consume_if_ident("readonly") {
            readonly = true;
        }
        if !self.consume_if_ident("attribute") {
            self.diag("expected 'attribute'");
            self.consume_trailing_semi();
            return None;
        }
        let ty = self.parse_type();
        let name = self.expect_ident("attribute name");
        self.consume_trailing_semi();
        Some(Member::Attribute {
            name,
            ty,
            readonly,
            doc: None,
        })
    }

    fn parse_const_member(&mut self) -> Option<Member> {
        let ty = self.parse_type();
        let name = self.expect_ident("constant name");
        self.expect_sym('=');
        let value = self.parse_const_value();
        self.consume_trailing_semi();
        Some(Member::Const {
            name,
            ty,
            value,
            doc: None,
        })
    }

    fn parse_member(&mut self, ctx: Container) -> Option<Member> {
        self.skip_extended_attrs();
        if self.consume_if_ident("const") {
            return self.parse_const_member();
        }
        if self.peek_is_attribute_start() {
            return self.parse_attribute();
        }
        if self.consume_if_ident("constructor") {
            self.expect_sym('(');
            let args = self.parse_args_body();
            self.consume_trailing_semi();
            return Some(Member::Constructor { args, doc: None });
        }

        // `iterable<K,V>`/`maplike<K,V>`/`setlike<T>` declare container
        // semantics — nothing maps into the port.
        if matches!(self.peek(), Token::Ident(s) if matches!(*s, "iterable" | "maplike" | "setlike"))
            && matches!(self.at(1), Token::Sym('<'))
        {
            self.advance();
            self.skip_generic_args();
            self.consume_trailing_semi();
            return None;
        }

        // `static` qualifies operations only — parse the rest as usual.
        self.consume_if_ident("static");

        // `getter`/`setter`/`deleter`/`legacycaller` qualify special
        // operations — parse them like ordinary ones.
        let _ = self.consume_if_ident("getter")
            || self.consume_if_ident("setter")
            || self.consume_if_ident("deleter")
            || self.consume_if_ident("legacycaller");

        if self.consume_if_ident("stringifier") {
            if self.consume_if_ident("attribute") {
                let ty = self.parse_type();
                let name = self.expect_ident("attribute name");
                self.consume_trailing_semi();
                return Some(Member::Attribute {
                    name,
                    ty,
                    readonly: true,
                    doc: None,
                });
            }
            // Bare `stringifier;` marks the default toString — nothing to emit.
            if self.match_sym(';') {
                return None;
            }
            // `stringifier <type> <name>(...)` falls through to operation parse.
        }

        if ctx == Container::Dictionary && self.consume_if_ident("required") {
            let ty = self.parse_type();
            let name = self.expect_ident("dictionary member name");
            if self.match_sym('=') {
                self.skip_arg_default();
            }
            self.consume_trailing_semi();
            return Some(Member::DictionaryField {
                name,
                ty,
                required: true,
                doc: None,
            });
        }

        if matches!(self.peek(), Token::Sym('}')) {
            return None;
        }

        // Try operation or dictionary field. The first token must be a type.
        if !self.peek_is_type() {
            self.diag(&format!(
                "unsupported member starting with {:?}",
                self.peek()
            ));
            self.consume_trailing_semi();
            return None;
        }

        let ty = self.parse_type();
        let name = self.expect_ident("member name");
        if self.match_sym('(') {
            let args = self.parse_args_body();
            self.consume_trailing_semi();
            return Some(Member::Operation {
                name,
                return_ty: ty,
                args,
                doc: None,
            });
        } else if ctx == Container::Dictionary {
            if self.match_sym('=') {
                self.skip_arg_default();
            }
            self.consume_trailing_semi();
            Some(Member::DictionaryField {
                name,
                ty,
                required: false,
                doc: None,
            })
        } else {
            self.diag("expected '(' for operation");
            self.consume_trailing_semi();
            None
        }
    }

    fn peek_is_type(&self) -> bool {
        match self.peek() {
            Token::Sym('(') => true,
            Token::Ident(_) => true,
            _ => false,
        }
    }

    fn parse_members(&mut self, ctx: Container) -> Vec<Member> {
        let mut members: Vec<Member> = Vec::new();
        loop {
            if let Some(t) = self.drain_comments()
                && let Some(last) = members.last_mut()
            {
                last.append_doc(&t);
            }
            self.skip_extended_attrs();
            if let Some(t) = self.drain_comments()
                && let Some(last) = members.last_mut()
            {
                last.append_doc(&t);
            }
            if matches!(self.peek(), Token::Sym('}') | Token::Eof) {
                self.pending.clear();
                break;
            }
            let start = self.pos;
            if let Some(mut m) = self.parse_member(ctx) {
                if let Some(doc) = self.take_pending() {
                    m.set_doc(doc);
                }
                members.push(m);
            }
            if self.pos == start {
                self.diag("stuck parsing member; advancing");
                self.advance();
            }
        }
        members
    }

    fn parse_interface(&mut self, partial: bool) -> AstDef {
        let name = self.expect_ident("interface name");
        let base = if self.match_sym(':') {
            Some(self.expect_ident("base interface"))
        } else {
            None
        };
        self.expect_sym('{');
        let members = self.parse_members(Container::Interface);
        self.expect_sym('}');
        AstDef::Interface {
            name,
            base,
            members,
            partial,
            doc: None,
        }
    }

    fn parse_mixin(&mut self, partial: bool) -> AstDef {
        let name = self.expect_ident("mixin name");
        self.expect_sym('{');
        let members = self.parse_members(Container::Mixin);
        self.expect_sym('}');
        AstDef::Mixin {
            name,
            members,
            partial,
            doc: None,
        }
    }

    fn parse_dictionary(&mut self, partial: bool) -> AstDef {
        let name = self.expect_ident("dictionary name");
        let base = if self.match_sym(':') {
            Some(self.expect_ident("base dictionary"))
        } else {
            None
        };
        self.expect_sym('{');
        let members = self.parse_members(Container::Dictionary);
        self.expect_sym('}');
        AstDef::Dictionary {
            name,
            base,
            members,
            partial,
            doc: None,
        }
    }

    fn parse_namespace(&mut self, partial: bool) -> AstDef {
        let name = self.expect_ident("namespace name");
        self.expect_sym('{');
        let members = self.parse_members(Container::Namespace);
        self.expect_sym('}');
        AstDef::Namespace {
            name,
            members,
            partial,
            doc: None,
        }
    }

    fn parse_enum(&mut self) -> AstDef {
        let name = self.expect_ident("enum name");
        self.expect_sym('{');
        let mut values: Vec<EnumValue> = Vec::new();
        loop {
            if let Some(t) = self.drain_comments()
                && let Some(last) = values.last_mut()
            {
                match &mut last.doc {
                    Some(d) => {
                        d.push('\n');
                        d.push_str(&t);
                    }
                    None => last.doc = Some(t),
                }
            }
            self.skip_extended_attrs();
            if self.match_sym('}') {
                self.pending.clear();
                break;
            }
            match self.peek() {
                Token::String(s) => {
                    let value = EnumValue {
                        name: (*s).to_string(),
                        doc: self.take_pending(),
                    };
                    values.push(value);
                    self.advance();
                }
                _ => {
                    self.diag("expected string enum value");
                    self.skip_to_semi();
                    break;
                }
            }
            if !self.match_sym(',') {
                if self.match_sym('}') {
                    break;
                }
                if !matches!(self.peek(), Token::Eof) {
                    self.skip_to_semi();
                }
                break;
            }
        }
        AstDef::Enum {
            name,
            values,
            doc: None,
        }
    }

    fn parse_typedef(&mut self) -> AstDef {
        let ty = self.parse_type();
        let name = self.expect_ident("typedef name");
        AstDef::Typedef {
            name,
            ty,
            doc: None,
        }
    }

    fn parse_callback(&mut self) -> AstDef {
        let _ = self.consume_if_ident("function");
        let name = self.expect_ident("callback name");
        let return_ty = if self.match_sym('=') {
            self.parse_type()
        } else if !matches!(self.peek(), Token::Sym('(')) {
            self.parse_type()
        } else {
            Type::Void
        };
        self.expect_sym('(');
        let args = self.parse_args_body();
        AstDef::Callback {
            name,
            return_ty,
            args,
            partial: false,
            doc: None,
        }
    }

    fn parse_definition(&mut self) -> Option<AstDef> {
        self.skip_extended_attrs();
        let partial = self.consume_if_ident("partial");
        if self.consume_if_ident("interface") {
            if self.consume_if_ident("mixin") {
                return Some(self.parse_mixin(partial));
            }
            return Some(self.parse_interface(partial));
        }
        if self.consume_if_ident("dictionary") {
            return Some(self.parse_dictionary(partial));
        }
        if self.consume_if_ident("enum") {
            return Some(self.parse_enum());
        }
        if self.consume_if_ident("typedef") {
            return Some(self.parse_typedef());
        }
        if self.consume_if_ident("callback") {
            // `callback interface X { ... }` — an interface whose single
            // operation is the callback; parse it as a plain interface.
            if self.consume_if_ident("interface") {
                return Some(self.parse_interface(partial));
            }
            return Some(self.parse_callback());
        }
        if self.consume_if_ident("namespace") {
            return Some(self.parse_namespace(partial));
        }
        if self.consume_if_ident("mixin") {
            return Some(self.parse_mixin(partial));
        }
        // `A includes B;` — mixin inclusion. The mixin's members already
        // emit as their own ports; record the statement and move on.
        if matches!(self.peek(), Token::Ident(_))
            && matches!(self.at(1), Token::Ident(s) if *s == "includes")
        {
            let target = self.expect_ident("includes target name");
            self.advance();
            let mixin = self.expect_ident("includes mixin name");
            return Some(AstDef::Skip(format!("{target} includes {mixin}")));
        }
        match self.peek() {
            Token::Ident(s) => self.diag(&format!("unsupported definition: {s}")),
            _ => self.diag("expected definition"),
        }
        self.skip_to_semi();
        None
    }

    pub fn parse(mut self) -> (Vec<AstDef>, Vec<String>) {
        let mut defs: Vec<AstDef> = Vec::new();
        while !matches!(self.peek(), Token::Eof) {
            if let Some(t) = self.drain_comments()
                && let Some(last) = defs.last_mut()
            {
                last.append_doc(&t);
            }
            self.skip_extended_attrs();
            if let Some(t) = self.drain_comments()
                && let Some(last) = defs.last_mut()
            {
                last.append_doc(&t);
            }
            if matches!(self.peek(), Token::Eof) {
                break;
            }
            // Drain the def-level doc BEFORE parsing — the container's
            // member loop would otherwise attribute it to the first
            // member.
            let def_doc = self.take_pending();
            let start = self.pos;
            if let Some(mut d) = self.parse_definition() {
                if let Some(doc) = def_doc {
                    d.set_doc(doc);
                }
                defs.push(d);
                if !self.match_sym(';') {
                    if !matches!(self.peek(), Token::Eof) {
                        self.diag("expected ';' after definition");
                        self.skip_to_semi();
                    }
                }
            }
            if self.pos == start {
                self.diag("stuck parsing definition; advancing");
                self.advance();
            }
        }
        (defs, self.diags)
    }
}

pub fn parse_webidl(src: &str) -> (Vec<AstDef>, Vec<String>) {
    let tokens = Tokenizer::new(src).tokenize();
    Parser::new(tokens).parse()
}

#[cfg(test)]
mod doc_tests {
    use super::*;

    /// Own-line comments before a definition become its doc; comments
    /// inside a container attach to their member.
    #[test]
    fn leading_comment_becomes_doc() {
        let (defs, diags) = parse_webidl(
            "/// Interface doc.\n[Exposed=Window]\ninterface Event {\n  /// Field doc.\n  readonly attribute DOMString type;\n};\n",
        );
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].doc(), Some("Interface doc."));
        let mut defs = defs;
        let members = defs[0].members_mut().unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].doc_mut().unwrap().as_deref(), Some("Field doc."),);
    }
}
