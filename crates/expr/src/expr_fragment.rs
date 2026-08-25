use crate::context::ParseContext;
use crate::token::Token;
use handl_ast_canonical::Expr;
use handl_lexer_canonical::CanonicalKind;
use handl_vm_values_json::NonFunctionValue;

/// A fragment that can parse a primary expression.
pub trait PrimaryFragment: Send + Sync {
    /// Returns the token kinds this fragment can parse as a primary.
    ///
    /// An empty slice means the fragment is a fallback that should be
    /// tried at any token position.
    fn primary_tokens(&self) -> &'static [CanonicalKind] {
        &[]
    }

    fn try_primary(
        &self,
        ctx: &mut ParseContext,
    ) -> Option<Result<Expr<NonFunctionValue>, handl_platform::PlatformError>>;
}

/// A fragment that can parse a postfix operator.
pub trait PostfixFragment: Send + Sync {
    /// Returns the token kinds this fragment can start a postfix with.
    ///
    /// An empty slice means the fragment is a fallback.
    fn postfix_tokens(&self) -> &'static [CanonicalKind] {
        &[]
    }

    fn try_postfix(
        &self,
        left: &mut Expr<NonFunctionValue>,
        ctx: &mut ParseContext,
    ) -> Option<Result<(), handl_platform::PlatformError>>;
}

/// A fragment that can parse an infix operator.
pub trait InfixFragment: Send + Sync {
    /// Returns the token kinds this fragment can parse as an infix.
    ///
    /// An empty slice means the fragment is a fallback.
    fn infix_tokens(&self) -> &'static [CanonicalKind] {
        &[]
    }

    /// Returns the left- and right-binding power for `token` if this
    /// fragment can handle it.
    fn binding(&self, token: &Token) -> Option<(u32, u32)>;

    /// Parses the infix expression whose operator has already been
    /// identified by `binding`. `left` is the already-parsed left-hand
    /// side, and is updated in place on success. Returns `Some(Ok(()))`
    /// on success, `Some(Err(_))` on a parse error, or `None` if this
    /// fragment does not apply.
    fn try_infix(
        &self,
        left: &mut Expr<NonFunctionValue>,
        ctx: &mut ParseContext,
    ) -> Option<Result<(), handl_platform::PlatformError>>;
}
