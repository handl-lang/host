use crate::*;

#[test]
fn level_constructions() {
    assert_eq!(Level::zero(), Level::Zero);
    assert_eq!(Level::zero().succ(), Level::Succ(Box::new(Level::Zero)));
    assert_eq!(
        Level::max(Level::zero().succ(), Level::zero()),
        Level::zero().succ()
    );
}

#[test]
fn level_ordering() {
    assert!(Level::zero().le(&Level::zero().succ()));
    assert!(Level::max(Level::zero(), Level::zero().succ()).le(&Level::zero().succ()));
    assert!(!Level::zero().succ().le(&Level::zero()));
    assert!(
        Level::zero()
            .succ()
            .le(&Level::max(Level::zero(), Level::zero().succ()))
    );
}

#[test]
fn identifiers_and_names() {
    let name = Name::new("x");
    assert_eq!(name, Name("x".to_string()));
    let _ = NameId(0);
    let _ = MetaVar(0);
    let _ = HoleId(0);
    let var = Var::DeBruijn(0);
    assert_eq!(var, Var::DeBruijn(0));
    assert_eq!(Var::Named("x".into()), Var::Named(Name::new("x")));
}

#[test]
fn env_lookup_and_extension() {
    let env = Env::new()
        .extend(Value::Literal(Literal::Int(1)))
        .extend(Value::Literal(Literal::Int(2)));
    assert_eq!(env.len(), 2);
    assert_eq!(
        env.lookup(0).cloned(),
        Some(Value::Literal(Literal::Int(2)))
    );
    assert_eq!(
        env.lookup(1).cloned(),
        Some(Value::Literal(Literal::Int(1)))
    );
    assert_eq!(env.lookup(2).cloned(), None);
}

#[test]
fn tctx_lookup_and_extension() {
    let tctx = TCtx::new()
        .extend(Name::new("x"), named_ty("X"))
        .extend(Name::new("y"), named_ty("Y"));
    assert_eq!(tctx.len(), 2);
    assert_eq!(tctx.lookup(0).cloned(), Some(named_ty("Y")));
    assert_eq!(tctx.lookup(1).cloned(), Some(named_ty("X")));
    assert_eq!(tctx.lookup(2).cloned(), None);
    let names = tctx.names();
    assert_eq!(names, vec![Name::new("y"), Name::new("x")]);
}

#[test]
fn eval_literal_and_universe() {
    assert_eq!(
        eval(&lit_bool(true), &Env::new()),
        Value::Literal(Literal::Bool(true))
    );
    assert_eq!(eval(&u0(), &Env::new()), Value::Universe(Level::zero()));
}

#[test]
fn eval_variable() {
    let env = Env::new().extend(Value::Literal(Literal::Int(42)));
    assert_eq!(eval(&bvar(0), &env), Value::Literal(Literal::Int(42)));
}

#[test]
fn eval_lambda_and_application() {
    let id = lam("x", bvar(0));
    let arg = lit_int(7);
    assert_eq!(
        eval(&app(id, arg), &Env::new()),
        Value::Literal(Literal::Int(7))
    );
}

#[test]
fn eval_let_binding() {
    let t = r#let("x", lit_int(5), bvar(0));
    assert_eq!(eval(&t, &Env::new()), Value::Literal(Literal::Int(5)));
}

#[test]
fn eval_pair_and_projection() {
    let t = pair(lit_int(1), lit_int(2));
    assert_eq!(
        eval(&proj1(t.clone()), &Env::new()),
        Value::Literal(Literal::Int(1))
    );
    assert_eq!(
        eval(&proj2(t), &Env::new()),
        Value::Literal(Literal::Int(2))
    );
}

#[test]
fn eval_stuck_application() {
    let f = var("f");
    let a = lit_int(3);
    assert_eq!(
        eval(&app(f, a), &Env::new()),
        Value::Neutral(Neutral::App {
            fun: Box::new(Neutral::Var(Var::Named("f".into()))),
            arg: Box::new(Value::Literal(Literal::Int(3))),
        })
    );
}

#[test]
fn apply_closure() {
    let v = eval(&lam("x", bvar(0)), &Env::new());
    if let Value::Lambda { closure, .. } = v {
        assert_eq!(
            closure.apply(Value::Literal(Literal::String("hi".to_string()))),
            Value::Literal(Literal::String("hi".to_string()))
        );
    } else {
        panic!("expected lambda value");
    }
}

#[test]
fn normalize_identity_lambda() {
    let id = lam("x", bvar(0));
    assert_eq!(normalize(&id), id);
}

#[test]
fn normalize_nested_lambda() {
    let t = lam("x", lam("y", bvar(1)));
    assert_eq!(normalize(&t), t);
}

#[test]
fn normalize_application_redex() {
    let t = app(lam("x", bvar(0)), lit_int(42));
    assert_eq!(normalize(&t), lit_int(42));
}

#[test]
fn normalize_pair_projection() {
    let t = proj1(pair(lit_int(10), lit_int(20)));
    assert_eq!(normalize(&t), lit_int(10));
}

#[test]
fn normalize_let() {
    let t = r#let("x", lit_int(99), bvar(0));
    assert_eq!(normalize(&t), lit_int(99));
}

#[test]
fn normalize_pi_and_sigma() {
    let p = pi("A", u0(), bvar(0));
    assert_eq!(normalize(&p), p);
    let s = sigma("A", u0(), bvar(0));
    assert_eq!(normalize(&s), s);
}

#[test]
fn quote_maps_named_bound_variables() {
    let v = eval(&lam("x", bvar(0)), &Env::new());
    assert_eq!(quote(&v, &[]), lam("x", bvar(0)));
}

#[test]
fn reduction_equal_and_convert() {
    assert!(equal(&u0(), &u0()));
    assert!(!equal(&u0(), &universe(Level::zero().succ())));
    assert!(convert(&u0(), &universe(Level::zero().succ())));
    assert!(!convert(&universe(Level::zero().succ()), &u0()));
}

#[test]
fn synth_universe_and_literal() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = TCtx::new();
    let ty = synth(&mut ctx, &env, &tctx, &u0()).unwrap();
    assert_eq!(quote(&ty, &[]), universe(Level::zero().succ()));

    let lit_ty = synth(&mut ctx, &env, &tctx, &lit_int(5)).unwrap();
    assert_eq!(quote(&lit_ty, &[]), var("Int"));
}

#[test]
fn synth_variable() {
    let mut ctx = MetaContext::new();
    let tctx = TCtx::new().extend(Name::new("x"), named_ty("Bool"));
    let ty = synth(&mut ctx, &Env::new(), &tctx, &bvar(0)).unwrap();
    assert_eq!(ty, named_ty("Bool"));
}

#[test]
fn synth_let_binding() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = TCtx::new();
    let t = r#let("x", lit_int(3), bvar(0));
    let ty = synth(&mut ctx, &env, &tctx, &t).unwrap();
    assert_eq!(ty, named_ty("Int"));
}

#[test]
fn synth_pi_universe() {
    let mut ctx = MetaContext::new();
    let t = pi("A", u0(), u0());
    let ty = synth(&mut ctx, &Env::new(), &TCtx::new(), &t).unwrap();
    assert_eq!(quote(&ty, &[]), universe(Level::zero().succ()));
}

#[test]
fn synth_dependent_pi() {
    let mut ctx = MetaContext::new();
    let t = pi("A", u0(), bvar(0));
    let ty = synth(&mut ctx, &Env::new(), &TCtx::new(), &t).unwrap();
    assert_eq!(
        quote(&ty, &[]),
        universe(Level::max(Level::zero().succ(), Level::zero()))
    );
}

#[test]
fn synth_app_with_pi_in_context() {
    let mut ctx = MetaContext::new();
    let tctx = TCtx::new().extend(
        Name::new("f"),
        Value::Pi {
            param_name: Name::new("x"),
            param: Box::new(named_ty("Int")),
            closure: Closure {
                env: Env::new(),
                name: Name::new("x"),
                body: var("Int"),
            },
        },
    );
    let ty = synth(&mut ctx, &Env::new(), &tctx, &app(bvar(0), lit_int(5))).unwrap();
    assert_eq!(ty, named_ty("Int"));
}

#[test]
fn synth_proj_with_sigma_in_context() {
    let mut ctx = MetaContext::new();
    let closure = TypeClosure {
        env: Env::new(),
        name: Name::new("x"),
        body: var("Int"),
    };
    let tctx = TCtx::new().extend(
        Name::new("p"),
        Value::Sigma {
            name: Name::new("x"),
            fst: Box::new(named_ty("Int")),
            closure,
        },
    );
    let ty1 = synth(&mut ctx, &Env::new(), &tctx, &proj1(bvar(0))).unwrap();
    assert_eq!(ty1, named_ty("Int"));
    let ty2 = synth(&mut ctx, &Env::new(), &tctx, &proj2(bvar(0))).unwrap();
    assert_eq!(ty2, named_ty("Int"));
}

#[test]
fn check_matches_expected() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = TCtx::new();
    let expected = Value::Universe(Level::zero().succ());
    assert!(check(&mut ctx, &env, &tctx, &u0(), &expected).is_ok());

    let expected = named_ty("Int");
    assert!(check(&mut ctx, &env, &tctx, &lit_int(5), &expected).is_ok());
}

#[test]
fn check_universe_cumulativity() {
    let mut ctx = MetaContext::new();
    let tctx = TCtx::new();
    assert!(
        check(
            &mut ctx,
            &Env::new(),
            &tctx,
            &u0(),
            &Value::Universe(Level::zero().succ().succ())
        )
        .is_ok()
    );
}

#[test]
fn check_mismatch_reports_error() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = TCtx::new();
    let expected = named_ty("String");
    let err = check(&mut ctx, &env, &tctx, &lit_int(5), &expected).unwrap_err();
    assert!(matches!(err, TypeError::TypeMismatch { .. }));
}

#[test]
fn check_lambda_against_pi() {
    let mut ctx = MetaContext::new();
    let tctx = TCtx::new();
    let int = named_ty("Int");
    let pi = Value::Pi {
        param_name: Name::new("x"),
        param: Box::new(int),
        closure: Closure {
            env: Env::new(),
            name: Name::new("x"),
            body: var("Int"),
        },
    };
    assert!(check(&mut ctx, &Env::new(), &tctx, &lam("x", bvar(0)), &pi).is_ok());
}

#[test]
fn check_pair_against_sigma() {
    let mut ctx = MetaContext::new();
    let tctx = TCtx::new();
    let int = named_ty("Int");
    let sigma = Value::Sigma {
        name: Name::new("x"),
        fst: Box::new(int),
        closure: TypeClosure {
            env: Env::new(),
            name: Name::new("x"),
            body: var("Int"),
        },
    };
    let p = pair(lit_int(1), lit_int(2));
    assert!(check(&mut ctx, &Env::new(), &tctx, &p, &sigma).is_ok());
}

#[test]
fn synth_failure_for_unannotated_lambda() {
    let mut ctx = MetaContext::new();
    let tctx = TCtx::new();
    let err = synth(&mut ctx, &Env::new(), &tctx, &lam("x", bvar(0))).unwrap_err();
    assert!(matches!(err, TypeError::CannotSynthesize(_)));
}

#[test]
fn synth_errors() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = TCtx::new();
    assert!(matches!(
        synth(&mut ctx, &env, &tctx, &var("x")).unwrap_err(),
        TypeError::ScopeError(_)
    ));
    assert!(matches!(
        synth(&mut ctx, &env, &tctx, &bvar(0)).unwrap_err(),
        TypeError::UndefinedVariable(_)
    ));
    assert!(matches!(
        synth(&mut ctx, &env, &tctx, &app(lit_int(1), lit_int(2))).unwrap_err(),
        TypeError::ExpectedFunction(_)
    ));
    assert!(matches!(
        synth(&mut ctx, &env, &tctx, &proj1(lit_int(1))).unwrap_err(),
        TypeError::ExpectedSigma(_)
    ));
    assert!(matches!(
        synth(&mut ctx, &env, &tctx, &pi("A", lit_int(1), bvar(0))).unwrap_err(),
        TypeError::UniverseMismatch(_)
    ));
}

#[test]
fn meta_context_and_substitution() {
    let mut ctx = MetaContext::new();
    let m1 = ctx.fresh_meta();
    let m2 = ctx.fresh_meta();
    assert_eq!(m1, MetaVar(0));
    assert_eq!(m2, MetaVar(1));

    let mut sub = Substitution::new();
    sub.insert(MetaVar(0), Value::Universe(Level::zero()));
    assert_eq!(
        sub.get(&MetaVar(0)).cloned(),
        Some(Value::Universe(Level::zero()))
    );
    assert_eq!(sub.get(&MetaVar(1)).cloned(), None);
}

#[test]
fn substitution_application() {
    let mut sub = Substitution::new();
    sub.insert(MetaVar(0), named_ty("A"));
    let v = Value::Neutral(Neutral::Meta(MetaVar(0)));
    assert_eq!(sub.apply_value(&v), named_ty("A"));
    assert_eq!(sub.apply_neutral(&Neutral::Meta(MetaVar(0))), named_ty("A"));
    let t = Term::Meta(MetaVar(0));
    assert_eq!(sub.apply_term(&t, &[]), var("A"));
}

#[test]
fn substitution_compose() {
    let mut s1 = Substitution::new();
    s1.insert(MetaVar(0), Value::Neutral(Neutral::Meta(MetaVar(1))));
    let mut s2 = Substitution::new();
    s2.insert(MetaVar(1), named_ty("A"));
    let composed = s1.compose(&s2);
    assert_eq!(
        composed.apply_value(&Value::Neutral(Neutral::Meta(MetaVar(0)))),
        named_ty("A")
    );
}

#[test]
fn unification_succeeds_for_equal_values() {
    let mut ctx = MetaContext::new();
    assert!(unify(&mut ctx, &named_ty("A"), &named_ty("A")).is_ok());
}

#[test]
fn unification_fails_for_different_rigid_values() {
    let mut ctx = MetaContext::new();
    assert!(unify(&mut ctx, &named_ty("A"), &named_ty("B")).is_err());
}

#[test]
fn unification_flex_rigid() {
    let mut ctx = MetaContext::new();
    let m = ctx.fresh_meta();
    assert!(unify(&mut ctx, &Value::Neutral(Neutral::Meta(m)), &named_ty("A")).is_ok());
    assert_eq!(ctx.subst.get(&m).cloned(), Some(named_ty("A")));
}

#[test]
fn unification_flex_flex() {
    let mut ctx = MetaContext::new();
    let m1 = ctx.fresh_meta();
    let m2 = ctx.fresh_meta();
    assert!(
        unify(
            &mut ctx,
            &Value::Neutral(Neutral::Meta(m1)),
            &Value::Neutral(Neutral::Meta(m2))
        )
        .is_ok()
    );
    assert_eq!(
        ctx.subst.get(&m1).cloned(),
        Some(Value::Neutral(Neutral::Meta(m2)))
    );
}

#[test]
fn unification_occurs_check() {
    let mut ctx = MetaContext::new();
    let m = ctx.fresh_meta();
    let bad = Value::Neutral(Neutral::App {
        fun: Box::new(Neutral::Meta(m)),
        arg: Box::new(Value::Literal(Literal::Int(1))),
    });
    let err = unify(&mut ctx, &Value::Neutral(Neutral::Meta(m)), &bad).unwrap_err();
    assert!(matches!(err, UnificationError::OccursCheck(0)));
}

#[test]
fn unification_pi_and_sigma() {
    let mut ctx = MetaContext::new();
    let p1 = Value::Pi {
        param_name: Name::new("x"),
        param: Box::new(named_ty("A")),
        closure: Closure {
            env: Env::new(),
            name: Name::new("x"),
            body: var("B"),
        },
    };
    let p2 = Value::Pi {
        param_name: Name::new("x"),
        param: Box::new(named_ty("A")),
        closure: Closure {
            env: Env::new(),
            name: Name::new("x"),
            body: var("B"),
        },
    };
    assert!(unify(&mut ctx, &p1, &p2).is_ok());

    let s1 = Value::Sigma {
        name: Name::new("x"),
        fst: Box::new(named_ty("A")),
        closure: TypeClosure {
            env: Env::new(),
            name: Name::new("x"),
            body: var("B"),
        },
    };
    let s2 = Value::Sigma {
        name: Name::new("x"),
        fst: Box::new(named_ty("A")),
        closure: TypeClosure {
            env: Env::new(),
            name: Name::new("x"),
            body: var("B"),
        },
    };
    assert!(unify(&mut ctx, &s1, &s2).is_ok());
}

#[test]
fn unification_lambda_and_pair() {
    let mut ctx = MetaContext::new();
    let l1 = Value::Lambda {
        name: Name::new("x"),
        closure: Closure {
            env: Env::new(),
            name: Name::new("x"),
            body: bvar(0),
        },
    };
    let l2 = Value::Lambda {
        name: Name::new("x"),
        closure: Closure {
            env: Env::new(),
            name: Name::new("x"),
            body: bvar(0),
        },
    };
    assert!(unify(&mut ctx, &l1, &l2).is_ok());

    let p1 = Value::Pair {
        fst: Box::new(Value::Literal(Literal::Int(1))),
        snd: Box::new(Value::Literal(Literal::Int(2))),
    };
    let p2 = Value::Pair {
        fst: Box::new(Value::Literal(Literal::Int(1))),
        snd: Box::new(Value::Literal(Literal::Int(2))),
    };
    assert!(unify(&mut ctx, &p1, &p2).is_ok());
}

#[test]
fn unification_literal_and_universe() {
    let mut ctx = MetaContext::new();
    assert!(
        unify(
            &mut ctx,
            &Value::Literal(Literal::Int(1)),
            &Value::Literal(Literal::Int(1))
        )
        .is_ok()
    );
    assert!(
        unify(
            &mut ctx,
            &Value::Literal(Literal::Int(1)),
            &Value::Literal(Literal::Int(2))
        )
        .is_err()
    );
    assert!(
        unify(
            &mut ctx,
            &Value::Universe(Level::zero()),
            &Value::Universe(Level::zero())
        )
        .is_ok()
    );
    assert!(
        unify(
            &mut ctx,
            &Value::Universe(Level::zero().succ()),
            &Value::Universe(Level::zero())
        )
        .is_err()
    );
}
