//! Lightweight WebIDL grammar for the SDK generator.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token<'a> {
    Ident(&'a str),
    Number(&'a str),
    String(&'a str),
    Sym(char),
    Eof,
}

struct Tokenizer<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> Tokenizer<'a> {
    fn new(src: &'a str) -> Self {
        Self { src, pos: 0 }
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

    fn skip_ws_comments(&mut self) {
        loop {
            if self.eof() {
                return;
            }
            if self.src[self.pos..].starts_with("//") {
                let rest = &self.src[self.pos..];
                if let Some(n) = rest.find('\n') {
                    self.pos += n + 1;
                } else {
                    self.pos = self.src.len();
                }
                continue;
            }
            if self.src[self.pos..].starts_with("/*") {
                let rest = &self.src[self.pos + 2..];
                if let Some(n) = rest.find("*/") {
                    self.pos += 2 + n + 2;
                } else {
                    self.pos = self.src.len();
                }
                continue;
            }
            let c = self.peek_char().unwrap();
            if c.is_whitespace() {
                self.advance_char();
                continue;
            }
            break;
        }
    }

    fn next_token(&mut self) -> Token<'a> {
        self.skip_ws_comments();
        if self.eof() {
            return Token::Eof;
        }
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
    },
    Attribute {
        name: String,
        ty: Type,
        readonly: bool,
    },
    DictionaryField {
        name: String,
        ty: Type,
        required: bool,
    },
    Operation {
        name: String,
        return_ty: Type,
        args: Vec<Argument>,
    },
    Constructor {
        args: Vec<Argument>,
    },
    Skip(String),
}

#[derive(Debug, Clone)]
pub enum AstDef {
    Interface {
        name: String,
        base: Option<String>,
        members: Vec<Member>,
        partial: bool,
    },
    Mixin {
        name: String,
        members: Vec<Member>,
        partial: bool,
    },
    Dictionary {
        name: String,
        base: Option<String>,
        members: Vec<Member>,
        partial: bool,
    },
    Namespace {
        name: String,
        members: Vec<Member>,
        partial: bool,
    },
    Enum {
        name: String,
        values: Vec<String>,
    },
    Typedef {
        name: String,
        ty: Type,
    },
    Callback {
        name: String,
        return_ty: Type,
        args: Vec<Argument>,
        partial: bool,
    },
    Skip(String),
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

    pub fn members_mut(&mut self) -> Option<&mut Vec<Member>> {
        match self {
            AstDef::Interface { members, .. }
            | AstDef::Mixin { members, .. }
            | AstDef::Dictionary { members, .. }
            | AstDef::Namespace { members, .. } => Some(members),
            _ => None,
        }
    }

    pub fn set_partial(&mut self, partial: bool) {
        match self {
            AstDef::Interface { partial: p, .. }
            | AstDef::Mixin { partial: p, .. }
            | AstDef::Dictionary { partial: p, .. }
            | AstDef::Namespace { partial: p, .. }
            | AstDef::Callback { partial: p, .. } => *p = partial,
            _ => {}
        }
    }

    pub fn merge(&mut self, mut other: AstDef, diags: &mut Vec<String>) {
        if self.kind() != other.kind() {
            diags.push(format!(
                "cannot merge partial {} into {}",
                other.kind(),
                self.kind()
            ));
            return;
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
}

impl<'a> Parser<'a> {
    fn new(tokens: Vec<Token<'a>>) -> Self {
        Self {
            tokens,
            pos: 0,
            diags: Vec::new(),
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
        while self.match_sym('[') {
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
                let r = (*s).parse::<i64>().unwrap_or(0);
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
        Some(Member::Attribute { name, ty, readonly })
    }

    fn parse_const_member(&mut self) -> Option<Member> {
        let ty = self.parse_type();
        let name = self.expect_ident("constant name");
        self.expect_sym('=');
        let value = self.parse_const_value();
        self.consume_trailing_semi();
        Some(Member::Const { name, ty, value })
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
            return Some(Member::Constructor { args });
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
        let mut members = Vec::new();
        loop {
            self.skip_extended_attrs();
            if matches!(self.peek(), Token::Sym('}') | Token::Eof) {
                break;
            }
            let start = self.pos;
            if let Some(m) = self.parse_member(ctx) {
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
        }
    }

    fn parse_enum(&mut self) -> AstDef {
        let name = self.expect_ident("enum name");
        self.expect_sym('{');
        let mut values = Vec::new();
        loop {
            self.skip_extended_attrs();
            if self.match_sym('}') {
                break;
            }
            match self.peek() {
                Token::String(s) => {
                    values.push((*s).to_string());
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
        AstDef::Enum { name, values }
    }

    fn parse_typedef(&mut self) -> AstDef {
        let ty = self.parse_type();
        let name = self.expect_ident("typedef name");
        AstDef::Typedef { name, ty }
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
            return Some(self.parse_callback());
        }
        if self.consume_if_ident("namespace") {
            return Some(self.parse_namespace(partial));
        }
        if self.consume_if_ident("mixin") {
            return Some(self.parse_mixin(partial));
        }
        match self.peek() {
            Token::Ident(s) => self.diag(&format!("unsupported definition: {s}")),
            _ => self.diag("expected definition"),
        }
        self.skip_to_semi();
        None
    }

    pub fn parse(mut self) -> (Vec<AstDef>, Vec<String>) {
        let mut defs = Vec::new();
        while !matches!(self.peek(), Token::Eof) {
            self.skip_extended_attrs();
            if matches!(self.peek(), Token::Eof) {
                break;
            }
            let start = self.pos;
            if let Some(d) = self.parse_definition() {
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
