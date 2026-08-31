//! Function-registration adapters: closure → [`Function`] / [`AsyncFunction`].
//!
//! These are the ADR 0040 replacements for `plugin-vm-common`'s `builder.rs`
//! adapters (`FnFunction`, `FnLazy`, `FnGeneric`, `FnAsync`), adapted to the
//! non-generic platform traits. They live here (in `plugin-ext-eval`) rather
//! than in `handl-platform` because they are dev/host plumbing, not core
//! abstractions — the traits themselves stay in `handl-platform`.
//!
//! # What's here
//!
//! - [`GenericFunction`] — a [`Function`] backed by a closure. This is the
//!   public-utility version of `handl-test-utils`'s `MockFunction`; it lets
//!   library/host code define inline functions without a one-off struct.
//! - [`LazyFunction`] — a [`Function`] that constructs its inner function on
//!   first call (lazy initialization) and caches it thereafter. Useful for
//!   expensive-to-construct functions.
//! - [`AsyncFn`] — an [`AsyncFunction`] (and [`Function`]) backed by a closure
//!   returning a boxed future. The async path is advertised via
//!   [`Function::as_async`] so [`EvalContext::call_function_async`] dispatches
//!   through `call_async`; the sync `call` returns an error indicating the
//!   function is async-only.

use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

use serde_json::Value;

use handl_platform::{AsyncFunction, EvalContext, EvalError, Function};

/// A boxed eval future: single-threaded (no `Send` bound), borrowing its inputs.
pub type EvalFuture<'a> = Pin<Box<dyn Future<Output = Result<Value, EvalError>> + 'a>>;

// ---------------------------------------------------------------------------
// GenericFunction
// ---------------------------------------------------------------------------

/// A [`Function`] backed by a closure.
///
/// The closure receives the argument slice and a mutable [`EvalContext`],
/// matching the [`Function::call`] signature. This lets library and host code
/// define inline functions without writing a one-off struct. It is the
/// public-utility analogue of `handl-test-utils`'s `MockFunction` (ported from
/// `plugin-vm-common`'s `FnGeneric` / `FnFunction`).
///
/// # Example
///
/// ```ignore
/// use handl_plugin_ext_eval::GenericFunction;
/// use handl_platform::Function;
/// use serde_json::{Value, json};
///
/// let add = GenericFunction::new("add", |args, _ctx| {
///     let a = args[0].as_i64().unwrap();
///     let b = args[1].as_i64().unwrap();
///     Ok(json!(a + b))
/// });
/// ```
pub struct GenericFunction {
    name: String,
    handler: Box<dyn Fn(&[Value], &mut EvalContext) -> Result<Value, EvalError> + Send + Sync>,
}

impl GenericFunction {
    /// Create a new generic function with the given name and handler closure.
    pub fn new(
        name: impl Into<String>,
        handler: impl Fn(&[Value], &mut EvalContext) -> Result<Value, EvalError> + Send + Sync + 'static,
    ) -> Self {
        GenericFunction {
            name: name.into(),
            handler: Box::new(handler),
        }
    }

    /// The function's name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Function for GenericFunction {
    fn call(&self, args: &[Value], ctx: &mut EvalContext) -> Result<Value, EvalError> {
        (self.handler)(args, ctx)
    }
}

// ---------------------------------------------------------------------------
// LazyFunction
// ---------------------------------------------------------------------------

/// A [`Function`] that constructs its inner function on first call and caches
/// it for subsequent calls.
///
/// This is the ADR 0040 replacement for `plugin-vm-common`'s `FnLazy` adapter,
/// reinterpreted as a lazy-initialization wrapper (the new system's eval rules
/// handle argument evaluation, so the old "receives unevaluated args" semantics
/// are no longer needed at the `Function` level). The init closure runs at most
/// once; afterwards every call delegates to the cached inner [`Function`].
///
/// Useful for functions that are expensive to construct (e.g. ones that build
/// a lookup table or open a resource on first use) but cheap to invoke.
///
/// # Example
///
/// ```ignore
/// use handl_plugin_ext_eval::{GenericFunction, LazyFunction};
/// use handl_platform::Function;
/// use serde_json::{Value, json};
///
/// let lazy = LazyFunction::new("expensive", || {
///     Box::new(GenericFunction::new("expensive", |args, _ctx| {
///         Ok(json!(args[0]))
///     }))
/// });
/// ```
pub struct LazyFunction {
    name: String,
    init: Mutex<Option<Box<dyn FnOnce() -> Box<dyn Function + Send + Sync> + Send + Sync>>>,
    cached: Mutex<Option<Box<dyn Function + Send + Sync>>>,
}

impl LazyFunction {
    /// Create a new lazy function.
    ///
    /// `init` is called the first time [`Function::call`] is invoked; its
    /// result is cached and reused for all subsequent calls.
    pub fn new(
        name: impl Into<String>,
        init: impl FnOnce() -> Box<dyn Function + Send + Sync> + Send + Sync + 'static,
    ) -> Self {
        LazyFunction {
            name: name.into(),
            init: Mutex::new(Some(Box::new(init))),
            cached: Mutex::new(None),
        }
    }

    /// The function's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns `true` if the inner function has been initialized.
    pub fn is_initialized(&self) -> bool {
        self.cached.lock().unwrap().is_some()
    }
}

impl Function for LazyFunction {
    fn call(&self, args: &[Value], ctx: &mut EvalContext) -> Result<Value, EvalError> {
        // Fast path: inner already initialized.
        {
            let cached = self.cached.lock().unwrap();
            if let Some(func) = cached.as_ref() {
                return func.call(args, ctx);
            }
        }
        // Slow path: take the init closure (runs at most once) and cache it.
        let init = self
            .init
            .lock()
            .unwrap()
            .take()
            .expect("LazyFunction init taken without producing a function");
        let func = init();
        let result = func.call(args, ctx);
        *self.cached.lock().unwrap() = Some(func);
        result
    }
}

// ---------------------------------------------------------------------------
// AsyncFn
// ---------------------------------------------------------------------------

/// An [`AsyncFunction`] backed by a closure that returns a boxed future.
///
/// The closure has a higher-ranked signature:
/// `for<'a> Fn(&'a [Value], &'a mut EvalContext) -> EvalFuture<'a>`, matching
/// [`AsyncFunction::call_async`]. This is the ADR 0040 replacement for
/// `plugin-vm-common`'s `FnAsync` adapter.
///
/// `AsyncFn` also implements [`Function`]: the sync [`Function::call`] returns
/// an [`EvalError::Other`] indicating the function is async-only, and
/// [`Function::as_async`] returns `Some(self)` so that
/// [`EvalContext::call_function_async`] dispatches through the async path.
/// This lets an `AsyncFn` be registered in a [`Module`](handl_platform::Module)
/// (which stores `Arc<dyn Function>`) and reached via async lookup.
///
/// # Example
///
/// ```ignore
/// use handl_plugin_ext_eval::AsyncFn;
/// use handl_platform::{AsyncFunction, EvalContext, EvalError};
/// use serde_json::{Value, json};
/// use std::future::ready;
///
/// let fetch = AsyncFn::new("fetch", |args, _ctx| {
///     Box::pin(ready(Ok(json!(format!("fetched:{}", args[0].as_str().unwrap())))))
/// });
/// ```
pub struct AsyncFn<F> {
    name: String,
    handler: F,
}

impl<F> AsyncFn<F>
where
    F: for<'a> Fn(&'a [Value], &'a mut EvalContext) -> EvalFuture<'a> + Send + Sync,
{
    /// Create a new async function with the given name and handler closure.
    ///
    /// The handler closure must be higher-ranked: for any lifetime `'a`, it
    /// takes `&'a [Value]` and `&'a mut EvalContext` and returns an
    /// [`EvalFuture<'a>`]. This bound is enforced here (at construction) so
    /// closure argument types are inferred correctly.
    pub fn new(name: impl Into<String>, handler: F) -> Self {
        AsyncFn {
            name: name.into(),
            handler,
        }
    }

    /// The function's name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl<F> Function for AsyncFn<F>
where
    F: for<'a> Fn(&'a [Value], &'a mut EvalContext) -> EvalFuture<'a> + Send + Sync + 'static,
{
    fn call(&self, _args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        Err(EvalError::Other(format!(
            "function `{}` is async-only — use call_function_async",
            self.name
        )))
    }

    fn as_async(&self) -> Option<&dyn AsyncFunction> {
        Some(self)
    }
}

impl<F> AsyncFunction for AsyncFn<F>
where
    F: for<'a> Fn(&'a [Value], &'a mut EvalContext) -> EvalFuture<'a> + Send + Sync + 'static,
{
    fn call_async<'a>(
        &'a self,
        args: &'a [Value],
        ctx: &'a mut EvalContext,
    ) -> Pin<Box<dyn Future<Output = Result<Value, EvalError>> + 'a>> {
        (self.handler)(args, ctx)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{EvalContext, EvalError, Function, Module, ModuleLookup};
    use handl_plugin_module::ModuleRegistry;
    use serde_json::{Value, json};
    use std::future::ready;
    use std::rc::Rc;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Box a future into an [`EvalFuture`] with the correct borrowed lifetime.
    /// This sidesteps HRTB return-type inference issues in test closures.
    fn box_fut<'a>(f: impl Future<Output = Result<Value, EvalError>> + 'a) -> EvalFuture<'a> {
        Box::pin(f)
    }

    /// Minimal single-threaded future executor (mirrors the one in
    /// `handl-platform`'s tests) so this crate's tests don't pull in an
    /// executor dependency.
    fn block_on<F: std::future::Future>(mut fut: F) -> F::Output {
        use std::pin::Pin;
        use std::sync::Arc;
        use std::task::{Context, Poll, Wake, Waker};

        struct NoopWake;
        impl Wake for NoopWake {
            fn wake(self: Arc<Self>) {}
        }
        let waker: Waker = Arc::new(NoopWake).into();
        let mut cx = Context::from_waker(&waker);
        let mut fut = unsafe { Pin::new_unchecked(&mut fut) };
        loop {
            match fut.as_mut().poll(&mut cx) {
                Poll::Ready(out) => return out,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    // --- GenericFunction ---

    #[test]
    fn generic_function_calls_closure() {
        let add = GenericFunction::new("add", |args, _ctx| {
            let a = args[0].as_i64().unwrap();
            let b = args[1].as_i64().unwrap();
            Ok(json!(a + b))
        });
        let mut ctx = EvalContext::new();
        let result = add.call(&[json!(2), json!(3)], &mut ctx).unwrap();
        assert_eq!(result, json!(5));
        assert_eq!(add.name(), "add");
    }

    // --- LazyFunction ---

    #[test]
    fn lazy_function_initializes_once_and_caches() {
        let init_count = Arc::new(AtomicUsize::new(0));
        let init_count_clone = init_count.clone();

        let lazy = LazyFunction::new("expensive", move || {
            init_count_clone.fetch_add(1, Ordering::SeqCst);
            Box::new(GenericFunction::new("expensive", |args, _ctx| {
                Ok(args.first().cloned().unwrap_or(Value::Null))
            }))
        });

        assert!(!lazy.is_initialized());
        assert_eq!(init_count.load(Ordering::SeqCst), 0);

        let mut ctx = EvalContext::new();
        let r1 = lazy.call(&[json!(1)], &mut ctx).unwrap();
        assert_eq!(r1, json!(1));
        assert!(lazy.is_initialized());
        assert_eq!(init_count.load(Ordering::SeqCst), 1);

        // Subsequent calls reuse the cached function — init runs only once.
        let r2 = lazy.call(&[json!(2)], &mut ctx).unwrap();
        let r3 = lazy.call(&[json!(3)], &mut ctx).unwrap();
        assert_eq!(r2, json!(2));
        assert_eq!(r3, json!(3));
        assert_eq!(init_count.load(Ordering::SeqCst), 1);
        assert_eq!(lazy.name(), "expensive");
    }

    // --- AsyncFn ---

    #[test]
    fn async_fn_call_async_returns_future() {
        let fetch = AsyncFn::new("fetch", |args, _ctx| {
            let v = args
                .first()
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            box_fut(ready(Ok(json!(format!("fetched:{v}")))))
        });

        let mut ctx = EvalContext::new();
        let args = [json!("x")];
        let fut = fetch.call_async(&args, &mut ctx);
        let result = block_on(fut).unwrap();
        assert_eq!(result, json!("fetched:x"));
        assert_eq!(fetch.name(), "fetch");
    }

    #[test]
    fn async_fn_sync_call_returns_async_only_error() {
        let fetch = AsyncFn::new("fetch", |_args, _ctx| box_fut(ready(Ok(json!(0)))));
        let mut ctx = EvalContext::new();
        let err = fetch.call(&[json!(1)], &mut ctx).unwrap_err();
        assert!(matches!(err, EvalError::Other(_)));
    }

    #[test]
    fn async_fn_advertises_async_via_as_async() {
        let fetch = AsyncFn::new("fetch", |_args, _ctx| box_fut(ready(Ok(json!(0)))));
        assert!(fetch.as_async().is_some());
    }

    // --- call_function_async integration ---

    struct EchoModule;
    impl Module for EchoModule {
        fn name(&self) -> &str {
            "echo"
        }
        fn function(&self, name: &str) -> Option<&dyn Function> {
            match name {
                "async_echo" => Some(&AsyncEchoFn),
                "sync_echo" => Some(&SyncEchoFn),
                _ => None,
            }
        }
    }

    struct AsyncEchoFn;
    impl Function for AsyncEchoFn {
        fn call(&self, _args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
            Err(EvalError::Other("async-only".into()))
        }
        fn as_async(&self) -> Option<&dyn AsyncFunction> {
            Some(self)
        }
    }
    impl AsyncFunction for AsyncEchoFn {
        fn call_async<'a>(
            &'a self,
            args: &'a [Value],
            _ctx: &'a mut EvalContext,
        ) -> Pin<Box<dyn Future<Output = Result<Value, EvalError>> + 'a>> {
            Box::pin(ready(Ok(args.first().cloned().unwrap_or(Value::Null))))
        }
    }

    struct SyncEchoFn;
    impl Function for SyncEchoFn {
        fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
            Ok(args.first().cloned().unwrap_or(Value::Null))
        }
    }

    #[test]
    fn call_function_async_dispatches_to_async_function() {
        let registry = ModuleRegistry::new().with(EchoModule);
        let lookup: Rc<dyn ModuleLookup> = Rc::new(registry);
        let mut ctx = EvalContext::new();
        ctx.set_modules(lookup);

        let args = [json!("hello")];
        let fut = ctx.call_function_async("echo.async_echo", &args);
        let result = block_on(fut).unwrap();
        assert_eq!(result, json!("hello"));
    }

    #[test]
    fn call_function_async_falls_back_to_sync_function() {
        let registry = ModuleRegistry::new().with(EchoModule);
        let lookup: Rc<dyn ModuleLookup> = Rc::new(registry);
        let mut ctx = EvalContext::new();
        ctx.set_modules(lookup);

        let args = [json!(42)];
        let fut = ctx.call_function_async("echo.sync_echo", &args);
        let result = block_on(fut).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn call_function_async_unknown_function() {
        let registry = ModuleRegistry::new().with(EchoModule);
        let lookup: Rc<dyn ModuleLookup> = Rc::new(registry);
        let mut ctx = EvalContext::new();
        ctx.set_modules(lookup);

        let args = [json!(1)];
        let fut = ctx.call_function_async("echo.missing", &args);
        let err = block_on(fut).unwrap_err();
        assert_eq!(err, EvalError::UnknownFunction("echo.missing".to_string()));
    }
}
