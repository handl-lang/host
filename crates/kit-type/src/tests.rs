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

#[test]
fn kind_constructions() {
    let k = Kind::Type;
    assert_eq!(k, Kind::Type);
    assert_eq!(k.normalize(), k);

    let arr = Kind::fun(Kind::Type, Kind::Type);
    assert_eq!(arr, Kind::fun(Kind::Type, Kind::Type));
    assert_eq!(arr.normalize(), arr);
    assert_eq!(format!("{}", arr), "(Type -> Type)");

    let nested = Kind::fun(Kind::fun(Kind::Type, Kind::Type), Kind::Type);
    assert_eq!(format!("{}", nested), "((Type -> Type) -> Type)");

    assert_eq!(kfun(Kind::Type, Kind::Type), arr);
}

#[test]
fn tctx_kind_lookup() {
    let tctx = TCtx::new()
        .extend_with_kind(Name::new("A"), named_ty("A"), Kind::Type)
        .extend_with_kind(
            Name::new("F"),
            named_ty("F"),
            Kind::fun(Kind::Type, Kind::Type),
        );
    assert_eq!(
        tctx.lookup_kind(0).cloned(),
        Some(Kind::fun(Kind::Type, Kind::Type))
    );
    assert_eq!(tctx.lookup_kind(1).cloned(), Some(Kind::Type));
    assert_eq!(
        tctx.lookup_kind_by_name(&Name::new("F")).cloned(),
        Some(Kind::fun(Kind::Type, Kind::Type))
    );
    assert_eq!(tctx.lookup(0).cloned(), Some(named_ty("F")));
}

#[test]
fn synth_kind_universe() {
    let mut ctx = MetaContext::new();
    let k = synth_kind(&mut ctx, &Env::new(), &TCtx::new(), &u0()).unwrap();
    assert_eq!(k, Kind::Type);
}

#[test]
fn synth_kind_type_lam_and_app() {
    let mut ctx = MetaContext::new();
    let id = type_lam("A", Kind::Type, bvar(0));
    let k = synth_kind(&mut ctx, &Env::new(), &TCtx::new(), &id).unwrap();
    assert_eq!(k, Kind::fun(Kind::Type, Kind::Type));

    let app = type_app(id, u0());
    let k2 = synth_kind(&mut ctx, &Env::new(), &TCtx::new(), &app).unwrap();
    assert_eq!(k2, Kind::Type);
}

#[test]
fn check_kind_hkt() {
    let mut ctx = MetaContext::new();
    let id = type_lam("A", Kind::Type, bvar(0));
    assert!(
        check_kind(
            &mut ctx,
            &Env::new(),
            &TCtx::new(),
            &id,
            &Kind::fun(Kind::Type, Kind::Type)
        )
        .is_ok()
    );

    let app = type_app(id, u0());
    assert!(check_kind(&mut ctx, &Env::new(), &TCtx::new(), &app, &Kind::Type).is_ok());
}

#[test]
fn kind_errors() {
    let mut ctx = MetaContext::new();
    let tctx = TCtx::new();

    // Applying a ground type as if it were a constructor.
    let bad = type_app(u0(), u0());
    assert!(matches!(
        synth_kind(&mut ctx, &Env::new(), &tctx, &bad).unwrap_err(),
        TypeError::ExpectedFunctionKind(_)
    ));

    // Argument kind mismatch.
    let id = type_lam("A", Kind::Type, bvar(0));
    let bad_arg = type_app(id.clone(), id.clone());
    assert!(matches!(
        synth_kind(&mut ctx, &Env::new(), &tctx, &bad_arg).unwrap_err(),
        TypeError::KindMismatch { .. }
    ));

    // Undefined named type variable.
    assert!(matches!(
        synth_kind(&mut ctx, &Env::new(), &tctx, &tvar("X")).unwrap_err(),
        TypeError::UndefinedTypeVariable(_)
    ));

    // Undefined bound type variable.
    assert!(matches!(
        synth_kind(&mut ctx, &Env::new(), &tctx, &bvar(0)).unwrap_err(),
        TypeError::UndefinedTypeVariable(_)
    ));
}

#[test]
fn eval_type_lam_and_application() {
    let id = type_lam("A", Kind::Type, bvar(0));
    let app = type_app(id, u0());
    assert_eq!(eval(&app, &Env::new()), Value::Universe(Level::zero()));
}

#[test]
fn eval_stuck_type_application() {
    let t = type_app(tvar("F"), u0());
    let val = eval(&t, &Env::new());
    assert_eq!(
        val,
        Value::Neutral(Neutral::TypeApp {
            fun: Box::new(Neutral::TVar(Name::new("F"))),
            arg: Box::new(Value::Universe(Level::zero())),
        })
    );
}

#[test]
fn normalize_type_lam_round_trip() {
    let id = type_lam("A", Kind::Type, bvar(0));
    assert_eq!(normalize(&id), id);
}

#[test]
fn normalize_hkt_redex() {
    let id = type_lam("A", Kind::Type, bvar(0));
    let app = type_app(id, u0());
    assert_eq!(normalize(&app), u0());
}

#[test]
fn hkt_unification_flex_rigid() {
    let mut ctx = MetaContext::new();
    let f = Value::Neutral(Neutral::TVar(Name::new("F")));
    let u = Value::Universe(Level::zero());
    assert!(unify(&mut ctx, &f, &u).is_ok());
    assert_eq!(
        ctx.subst.get_tvar(&Name::new("F")).cloned(),
        Some(Value::Universe(Level::zero()))
    );
}

#[test]
fn hkt_unification_type_lam() {
    let mut ctx = MetaContext::new();
    let id1 = eval(&type_lam("A", Kind::Type, bvar(0)), &Env::new());
    let id2 = eval(&type_lam("B", Kind::Type, bvar(0)), &Env::new());
    assert!(unify(&mut ctx, &id1, &id2).is_ok());
}

#[test]
fn hkt_unification_tvar_occurs() {
    let mut ctx = MetaContext::new();
    let f = Value::Neutral(Neutral::TVar(Name::new("F")));
    let app = Value::Neutral(Neutral::TypeApp {
        fun: Box::new(Neutral::TVar(Name::new("F"))),
        arg: Box::new(Value::Universe(Level::zero())),
    });
    assert!(matches!(
        unify(&mut ctx, &f, &app).unwrap_err(),
        UnificationError::TVarOccursCheck(_)
    ));
}

#[test]
fn tvar_substitution_application() {
    let mut sub = Substitution::new();
    sub.insert_tvar(Name::new("F"), Value::Universe(Level::zero()));

    let v = Value::Neutral(Neutral::TVar(Name::new("F")));
    assert_eq!(sub.apply_value(&v), Value::Universe(Level::zero()));

    let t = tvar("F");
    assert_eq!(sub.apply_term(&t, &[]), u0());
}

#[test]
fn type_lam_quote_neutral() {
    let id = eval(&type_lam("A", Kind::Type, bvar(0)), &Env::new());
    let q = quote(&id, &[]);
    assert_eq!(q, type_lam("A", Kind::Type, bvar(0)));
}

#[test]
fn synth_type_for_alias_and_tref() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = TCtx::new()
        .extend_with_kind(
            Name::new("Int"),
            Value::Neutral(Neutral::TVar(Name::new("Int"))),
            Kind::Type,
        )
        .add_alias(Name::new("MyInt"), tvar("Int"), Kind::Type);
    let ty = synth_type(&mut ctx, &env, &tctx, &tref("MyInt")).unwrap();
    assert_eq!(ty, Value::Neutral(Neutral::TVar(Name::new("Int"))));
}

#[test]
fn alias_depth_limit() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = TCtx::new().add_alias(Name::new("A"), tref("A"), Kind::Type);
    let err = synth_type(&mut ctx, &env, &tctx, &tref("A")).unwrap_err();
    assert!(matches!(err, TypeError::AliasCycle(_)));
}

#[test]
fn alias_in_pi_param() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = TCtx::new()
        .extend_with_kind(
            Name::new("Int"),
            Value::Neutral(Neutral::TVar(Name::new("Int"))),
            Kind::Type,
        )
        .add_alias(Name::new("MyInt"), tvar("Int"), Kind::Type);
    let ty = synth_type(&mut ctx, &env, &tctx, &pi("x", tref("MyInt"), bvar(0))).unwrap();
    assert_eq!(quote(&ty, &[]), pi("x", tvar("Int"), bvar(0)));
}

#[test]
fn alias_in_sigma_param() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = TCtx::new()
        .extend_with_kind(
            Name::new("Int"),
            Value::Neutral(Neutral::TVar(Name::new("Int"))),
            Kind::Type,
        )
        .add_alias(Name::new("MyInt"), tvar("Int"), Kind::Type);
    let ty = synth_type(&mut ctx, &env, &tctx, &sigma("x", tref("MyInt"), bvar(0))).unwrap();
    assert_eq!(quote(&ty, &[]), sigma("x", tvar("Int"), bvar(0)));
}

#[test]
fn generic_alias_expansion() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    // Pair<T> = Sigma { x: T, y: T }
    let pair_ty = type_lam("T", Kind::Type, sigma("x", bvar(0), bvar(1)));
    let tctx = TCtx::new()
        .extend_with_kind(
            Name::new("Int"),
            Value::Neutral(Neutral::TVar(Name::new("Int"))),
            Kind::Type,
        )
        .add_alias(
            Name::new("Pair"),
            pair_ty,
            Kind::fun(Kind::Type, Kind::Type),
        );
    let ty = synth_type(&mut ctx, &env, &tctx, &tref("Pair")).unwrap();
    assert!(matches!(ty, Value::TypeLam { .. }));
    // Pair<Int> synthesizes to Sigma(x:Int, Int)
    let app = type_app(tref("Pair"), tvar("Int"));
    let applied = synth_type(&mut ctx, &env, &tctx, &app).unwrap();
    assert_eq!(quote(&applied, &[]), sigma("x", tvar("Int"), tvar("Int")));
}

#[test]
fn generic_identity_check() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = TCtx::new();

    // id : ∀A. (x:A) -> A
    let id_type = forall(
        "A",
        Kind::Type,
        val_pi(
            "x",
            Value::Neutral(Neutral::TVar(Name::new("A"))),
            Env::new(),
            Name::new("x"),
            tvar("A"),
        ),
    );

    let id = gen_lam([("A", Kind::Type)], lam("x", bvar(0)));
    assert!(check(&mut ctx, &env, &tctx, &id, &id_type).is_ok());
}

#[test]
fn generic_apply_check() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = TCtx::new();

    // apply : ∀A. ∀B. (f:(x:A)->B) -> (x:A) -> B
    let a = Value::Neutral(Neutral::TVar(Name::new("A")));
    let _b = Value::Neutral(Neutral::TVar(Name::new("B")));
    let a_to_b = val_pi("x", a, Env::new(), Name::new("x"), tvar("B"));
    let apply_type = forall(
        "A",
        Kind::Type,
        forall(
            "B",
            Kind::Type,
            val_pi(
                "f",
                a_to_b.clone(),
                Env::new(),
                Name::new("f"),
                quote(&a_to_b, &[]),
            ),
        ),
    );

    let apply = gen_lam(
        [("A", Kind::Type), ("B", Kind::Type)],
        lam("f", lam("x", app(bvar(1), bvar(0)))),
    );
    assert!(check(&mut ctx, &env, &tctx, &apply, &apply_type).is_ok());
}

#[test]
fn generic_map_check() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = TCtx::new();

    let t = Value::Neutral(Neutral::TVar(Name::new("T")));
    let _u = Value::Neutral(Neutral::TVar(Name::new("U")));
    let t_to_u = val_pi("_", t, Env::new(), Name::new("_"), tvar("U"));
    // map : ∀T. ∀U. (T -> U) -> T -> U
    let map_type = forall(
        "T",
        Kind::Type,
        forall(
            "U",
            Kind::Type,
            val_pi(
                "f",
                t_to_u.clone(),
                Env::new(),
                Name::new("f"),
                quote(&t_to_u, &[]),
            ),
        ),
    );

    let map = gen_lam(
        [("T", Kind::Type), ("U", Kind::Type)],
        lam("f", lam("x", app(bvar(1), bvar(0)))),
    );
    assert!(check(&mut ctx, &env, &tctx, &map, &map_type).is_ok());
}

#[test]
fn generic_type_application() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = TCtx::new()
        .extend_with_kind(
            Name::new("Int"),
            Value::Neutral(Neutral::TVar(Name::new("Int"))),
            Kind::Type,
        )
        .extend(
            Name::new("id"),
            forall(
                "A",
                Kind::Type,
                val_pi(
                    "x",
                    Value::Neutral(Neutral::TVar(Name::new("A"))),
                    Env::new(),
                    Name::new("x"),
                    tvar("A"),
                ),
            ),
        );
    // id[Int] : (x:Int) -> Int
    let applied = gen_app(bvar(0), [tvar("Int")]);
    let ty = synth(&mut ctx, &env, &tctx, &applied).unwrap();
    assert_eq!(quote(&ty, &[]), pi("x", tvar("Int"), tvar("Int")));
}

#[test]
fn generic_identity_application() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = TCtx::new()
        .extend_with_kind(
            Name::new("Int"),
            Value::Neutral(Neutral::TVar(Name::new("Int"))),
            Kind::Type,
        )
        .extend(
            Name::new("id"),
            forall(
                "A",
                Kind::Type,
                val_pi(
                    "x",
                    Value::Neutral(Neutral::TVar(Name::new("A"))),
                    Env::new(),
                    Name::new("x"),
                    tvar("A"),
                ),
            ),
        );
    // id[Int] 5 : Int
    let term = app(gen_app(bvar(0), [tvar("Int")]), lit_int(5));
    let ty = synth(&mut ctx, &env, &tctx, &term).unwrap();
    assert_eq!(ty, Value::Neutral(Neutral::TVar(Name::new("Int"))));
}

#[test]
fn generic_kind_mismatch() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = TCtx::new().extend(
        Name::new("id"),
        forall(
            "A",
            Kind::Type,
            val_pi(
                "x",
                Value::Neutral(Neutral::TVar(Name::new("A"))),
                Env::new(),
                Name::new("x"),
                tvar("A"),
            ),
        ),
    );
    let bad = gen_app(bvar(0), [type_lam("A", Kind::Type, bvar(0))]);
    let err = synth(&mut ctx, &env, &tctx, &bad).unwrap_err();
    assert!(matches!(err, TypeError::KindMismatch { .. }));
}

#[test]
fn undefined_alias_error() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = TCtx::new();
    let err = synth(&mut ctx, &env, &tctx, &tref("Missing")).unwrap_err();
    assert!(matches!(err, TypeError::AliasNotFound(_)));
}

#[test]
fn forall_quote_round_trip() {
    let id_type = forall(
        "A",
        Kind::Type,
        val_pi(
            "x",
            Value::Neutral(Neutral::TVar(Name::new("A"))),
            Env::new(),
            Name::new("x"),
            tvar("A"),
        ),
    );
    let q = quote(&id_type, &[]);
    assert_eq!(q, type_lam("A", Kind::Type, pi("x", bvar(0), bvar(1))));
}

#[test]
fn level_max_and_core_api() {
    let a = Level::zero().succ();
    let b = Level::zero().succ();
    assert_eq!(Level::max(a, b), Level::zero().succ());
    let m = Level::max(Level::zero().succ(), Level::zero());
    assert_eq!(m, Level::zero().succ());
    let c = Level::Max(Box::new(Level::zero()), Box::new(Level::zero().succ()));
    assert!(Level::zero().le(&c));
    assert!(Level::zero().succ().le(&c));
}

#[test]
fn name_and_var_display() {
    assert_eq!(Name::new("x").to_string(), "x");
    assert_eq!(Var::DeBruijn(0).to_string(), "#0");
    assert_eq!(Var::Named("y".into()).to_string(), "y");
    assert_eq!(format!("{}", Kind::Type), "Type");
}

#[test]
fn env_and_tctx_methods() {
    let env = Env::new();
    assert!(env.is_empty());
    assert_eq!(env.entries().len(), 0);
    let env = env.extend(Value::Literal(Literal::Int(1)));
    assert!(!env.is_empty());
    let tctx = TCtx::new();
    assert!(tctx.is_empty());
    let tctx = tctx
        .extend_with_kind(Name::new("A"), named_ty("A"), Kind::Type)
        .add_alias(Name::new("A"), tvar("A"), Kind::Type);
    assert_eq!(tctx.lookup_kind(0).cloned(), Some(Kind::Type));
    assert!(tctx.lookup_alias(&Name::new("A")).is_some());
    assert_eq!(
        tctx.lookup_by_name(&Name::new("A")).cloned(),
        Some(named_ty("A"))
    );
}

#[test]
fn substitution_pair_forall_and_term() {
    let mut sub = Substitution::new();
    sub.insert_tvar(Name::new("A"), named_ty("A"));
    let pair = Value::Pair {
        fst: Box::new(Value::Neutral(Neutral::TVar(Name::new("A")))),
        snd: Box::new(Value::Neutral(Neutral::TVar(Name::new("A")))),
    };
    let applied = sub.apply_value(&pair);
    assert!(matches!(applied, Value::Pair { .. }));

    let fa = Value::Forall {
        param_name: Name::new("A"),
        param_kind: Kind::Type,
        body: Box::new(Value::Neutral(Neutral::TVar(Name::new("A")))),
    };
    assert_eq!(sub.apply_value(&fa), fa);

    let t = Term::Pair {
        fst: Box::new(Term::TVar(Name::new("A"))),
        snd: Box::new(Term::TVar(Name::new("A"))),
    };
    assert_eq!(
        sub.apply_term(&t, &[]),
        Term::Pair {
            fst: Box::new(var("A")),
            snd: Box::new(var("A"))
        }
    );
}

#[test]
fn instantiate_tvar_pair_and_forall() {
    let body = Value::Pair {
        fst: Box::new(Value::Neutral(Neutral::TVar(Name::new("A")))),
        snd: Box::new(Value::Literal(Literal::Int(1))),
    };
    let repl = named_ty("B");
    let q = instantiate_tvar_value(&body, &Name::new("A"), &repl);
    assert!(matches!(q, Value::Pair { .. }));

    let fa = Value::Forall {
        param_name: Name::new("A"),
        param_kind: Kind::Type,
        body: Box::new(Value::Neutral(Neutral::TVar(Name::new("A")))),
    };
    assert_eq!(instantiate_tvar_value(&fa, &Name::new("A"), &repl), fa);
}

#[test]
fn unification_edge_cases() {
    let mut ctx = MetaContext::new();

    let f1 = forall("A", Kind::Type, named_ty("A"));
    let f2 = forall("A", Kind::fun(Kind::Type, Kind::Type), named_ty("A"));
    assert!(matches!(
        unify(&mut ctx, &f1, &f2).unwrap_err(),
        UnificationError::KindMismatch { .. }
    ));

    let t1 = eval(&type_lam("A", Kind::Type, bvar(0)), &Env::new());
    let t2 = eval(
        &type_lam("A", Kind::fun(Kind::Type, Kind::Type), bvar(0)),
        &Env::new(),
    );
    assert!(matches!(
        unify(&mut ctx, &t1, &t2).unwrap_err(),
        UnificationError::KindMismatch { .. }
    ));

    let p1 = val_pi("x", named_ty("A"), Env::new(), Name::new("x"), var("A"));
    let p2 = val_pi("x", named_ty("B"), Env::new(), Name::new("x"), var("A"));
    assert!(matches!(
        unify(&mut ctx, &p1, &p2).unwrap_err(),
        UnificationError::Mismatch(_, _)
    ));

    let pair1 = Value::Pair {
        fst: Box::new(Value::Literal(Literal::Int(1))),
        snd: Box::new(Value::Literal(Literal::Int(2))),
    };
    let pair2 = Value::Pair {
        fst: Box::new(Value::Literal(Literal::Int(1))),
        snd: Box::new(Value::Literal(Literal::Int(3))),
    };
    assert!(unify(&mut ctx, &pair1, &pair2).is_err());

    assert!(
        unify(
            &mut ctx,
            &Value::Neutral(Neutral::Hole(HoleId(0))),
            &Value::Neutral(Neutral::Hole(HoleId(1)))
        )
        .is_err()
    );

    assert!(
        unify(
            &mut ctx,
            &Value::Universe(Level::zero()),
            &Value::Universe(Level::zero().succ())
        )
        .is_err()
    );

    let f = Value::Neutral(Neutral::TVar(Name::new("F")));
    let app = Value::Neutral(Neutral::TypeApp {
        fun: Box::new(Neutral::TVar(Name::new("F"))),
        arg: Box::new(Value::Universe(Level::zero())),
    });
    assert!(matches!(
        unify(&mut ctx, &f, &app).unwrap_err(),
        UnificationError::TVarOccursCheck(_)
    ));

    let m = ctx.fresh_meta();
    assert!(
        unify(
            &mut ctx,
            &Value::Neutral(Neutral::Meta(m)),
            &Value::Neutral(Neutral::Meta(m))
        )
        .is_ok()
    );
}

#[test]
fn synth_check_and_error_variants() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = TCtx::new();

    for t in [
        pair(lit_int(1), lit_int(2)),
        meta(0),
        hole(0),
        type_lam("A", Kind::Type, bvar(0)),
        lam("x", bvar(0)),
    ] {
        assert!(matches!(
            synth(&mut ctx, &env, &tctx, &t).unwrap_err(),
            TypeError::CannotSynthesize(_)
        ));
    }

    let bad = type_app(u0(), u0());
    assert!(matches!(
        synth_type(&mut ctx, &env, &tctx, &bad).unwrap_err(),
        TypeError::ExpectedFunctionKind(_)
    ));

    assert!(matches!(
        synth_type(&mut ctx, &env, &tctx, &tvar("X")).unwrap_err(),
        TypeError::UndefinedTypeVariable(_)
    ));

    let gen_lam_term = gen_lam([("A", Kind::Type)], lit_int(1));
    assert!(matches!(
        check(&mut ctx, &env, &tctx, &gen_lam_term, &named_ty("Int")).unwrap_err(),
        TypeError::TypeMismatch { .. }
    ));
}

#[test]
fn error_display_and_partial_eq() {
    let e = TypeError::CannotSynthesize("x".into());
    assert_eq!(e.to_string(), "cannot synthesize type for x");
    let e = TypeError::TypeMismatch {
        expected: "A".into(),
        found: "B".into(),
    };
    assert!(e.to_string().contains("type mismatch"));
    let e = TypeError::Unification(UnificationError::OccursCheck(0));
    assert!(e.to_string().contains("occurs check"));
    let e = UnificationError::LevelMismatch(Level::zero(), Level::zero().succ());
    assert!(e.to_string().contains("cannot unify levels"));
}

#[test]
fn quote_neutral_meta_hole_and_bound_tvar() {
    let v = Value::Neutral(Neutral::Meta(MetaVar(0)));
    assert_eq!(quote(&v, &[]), Term::Meta(MetaVar(0)));
    let v = Value::Neutral(Neutral::Hole(HoleId(0)));
    assert_eq!(quote(&v, &[]), Term::Hole(HoleId(0)));
    let v = Value::Neutral(Neutral::TVar(Name::new("A")));
    assert_eq!(quote(&v, &[Name::new("A")]), bvar(0));
    assert_eq!(quote(&v, &[]), tvar("A"));
}

#[test]
#[should_panic(expected = "application of non-function")]
fn apply_non_function_panics() {
    apply(
        Value::Universe(Level::zero()),
        Value::Literal(Literal::Int(1)),
    );
}

#[test]
#[should_panic(expected = "type application of non-type-constructor")]
fn apply_type_non_constructor_panics() {
    apply_type(
        Value::Literal(Literal::Int(1)),
        Value::Universe(Level::zero()),
    );
}

#[test]
#[should_panic(expected = "generic application of non-forall")]
fn apply_forall_non_forall_panics() {
    let tl = eval(&type_lam("A", Kind::Type, bvar(0)), &Env::new());
    apply_forall(tl, Value::Universe(Level::zero()));
}

#[test]
#[should_panic(expected = "proj1 on non-pair")]
fn proj1_non_pair_panics() {
    let _ = eval(&proj1(lit_int(1)), &Env::new());
}

#[test]
#[should_panic(expected = "proj2 on non-pair")]
fn proj2_non_pair_panics() {
    let _ = eval(&proj2(lit_int(1)), &Env::new());
}

#[test]
fn type_and_value_constructors() {
    let _ = kind_type();
    let _ = kfun(Kind::Type, Kind::Type);
    let _ = gen_lam([("A", Kind::Type)], bvar(0));
    let _ = gen_app(tvar("f"), [tvar("A")]);
    let _ = val_pi(
        "x",
        named_ty("Int"),
        Env::new(),
        Name::new("x"),
        tvar("Int"),
    );
    let _ = r#let("x", lit_int(1), bvar(0));
    let _ = apply_type(named_ty("Array"), named_ty("String"));
}

#[test]
fn convert_and_equal_non_universe() {
    assert!(!equal(&var("A"), &var("B")));
    assert!(!convert(&var("A"), &var("B")));
    assert!(convert(&u0(), &universe(Level::zero().succ())));
    assert!(!convert(&universe(Level::zero().succ()), &u0()));
}

#[test]
fn substitution_lambda_sigma_and_neutral_projection() {
    let mut sub = Substitution::new();
    sub.insert_tvar(Name::new("A"), named_ty("A"));
    let lam = Value::Lambda {
        name: Name::new("x"),
        closure: Closure {
            env: Env::new(),
            name: Name::new("x"),
            body: tvar("A"),
        },
    };
    let q = sub.apply_value(&lam);
    assert!(matches!(q, Value::Lambda { .. }));

    let sigma = Value::Sigma {
        name: Name::new("x"),
        fst: Box::new(Value::Neutral(Neutral::TVar(Name::new("A")))),
        closure: TypeClosure {
            env: Env::new(),
            name: Name::new("x"),
            body: tvar("A"),
        },
    };
    let q = sub.apply_value(&sigma);
    assert!(matches!(q, Value::Sigma { .. }));

    let p = Value::Pair {
        fst: Box::new(named_ty("A")),
        snd: Box::new(named_ty("B")),
    };
    let proj = Neutral::Proj1(Box::new(Neutral::App {
        fun: Box::new(Neutral::Var(Var::Named(Name::new("f")))),
        arg: Box::new(p),
    }));
    let r = sub.apply_neutral(&proj);
    assert!(matches!(r, Value::Neutral(_)));
}

#[test]
fn synth_genapp_and_let() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = TCtx::new().extend(Name::new("f"), named_ty("f"));
    let bad = gen_app(bvar(0), [tvar("A")]);
    assert!(matches!(
        synth(&mut ctx, &env, &tctx, &bad).unwrap_err(),
        TypeError::ExpectedFunction(_) | TypeError::CannotSynthesize(_)
    ));

    let t = r#let("x", lit_int(1), bvar(0));
    let ty = synth(&mut ctx, &Env::new(), &TCtx::new(), &t).unwrap();
    assert_eq!(ty, named_ty("Int"));
}

#[test]
fn eval_more_branches() {
    let env = Env::new();
    assert_eq!(
        eval(&alias("A", u0(), u0()), &env),
        Value::Universe(Level::zero())
    );

    let id_poly = eval(
        &gen_lam([("A", Kind::Type), ("B", Kind::Type)], bvar(1)),
        &env,
    );
    assert!(matches!(id_poly, Value::Forall { .. }));

    let app = gen_app(gen_lam([("A", Kind::Type)], tvar("A")), [u0()]);
    assert_eq!(eval(&app, &env), Value::Universe(Level::zero()));

    let p1 = eval(&proj1(var("p")), &env);
    assert!(matches!(p1, Value::Neutral(Neutral::Proj1(_))));
    let p2 = eval(&proj2(var("p")), &env);
    assert!(matches!(p2, Value::Neutral(Neutral::Proj2(_))));

    assert_eq!(
        eval(&meta(0), &env),
        Value::Neutral(Neutral::Meta(MetaVar(0)))
    );
    assert_eq!(
        eval(&hole(0), &env),
        Value::Neutral(Neutral::Hole(HoleId(0)))
    );
}

#[test]
fn apply_type_and_forall_branches() {
    let id = eval(&type_lam("A", Kind::Type, bvar(0)), &Env::new());
    assert_eq!(
        apply_type(id, Value::Universe(Level::zero())),
        Value::Universe(Level::zero())
    );

    let fa = forall(
        "A",
        Kind::Type,
        Value::Neutral(Neutral::TVar(Name::new("A"))),
    );
    let applied = apply_forall(fa, Value::Universe(Level::zero()));
    assert_eq!(applied, Value::Universe(Level::zero()));

    let stuck = apply_forall(Value::Neutral(Neutral::TVar(Name::new("F"))), named_ty("A"));
    assert!(matches!(stuck, Value::Neutral(Neutral::TypeApp { .. })));
}

#[test]
fn instantiate_tvar_branches() {
    let k = Kind::Type.clone();
    let ty_lam = Value::TypeLam {
        param_name: Name::new("A"),
        param_kind: k.clone(),
        closure: Closure {
            env: Env::new(),
            name: Name::new("A"),
            body: Term::TVar(Name::new("B")),
        },
    };
    let r = instantiate_tvar_value(&ty_lam, &Name::new("B"), &named_ty("C"));
    assert!(matches!(r, Value::TypeLam { .. }));

    let fa = forall(
        "B",
        k.clone(),
        Value::Neutral(Neutral::TVar(Name::new("C"))),
    );
    let r = instantiate_tvar_value(&fa, &Name::new("C"), &named_ty("D"));
    assert_eq!(r, forall("B", k.clone(), named_ty("D")));

    let pi = val_pi(
        "x",
        Value::Neutral(Neutral::TVar(Name::new("B"))),
        Env::new(),
        Name::new("x"),
        Term::TVar(Name::new("B")),
    );
    let r = instantiate_tvar_value(&pi, &Name::new("B"), &named_ty("C"));
    assert!(matches!(r, Value::Pi { .. }));

    let lam = Value::Lambda {
        name: Name::new("x"),
        closure: Closure {
            env: Env::new(),
            name: Name::new("x"),
            body: Term::TVar(Name::new("B")),
        },
    };
    let r = instantiate_tvar_value(&lam, &Name::new("B"), &named_ty("C"));
    assert!(matches!(r, Value::Lambda { .. }));

    let sigma = Value::Sigma {
        name: Name::new("x"),
        fst: Box::new(Value::Neutral(Neutral::TVar(Name::new("B")))),
        closure: TypeClosure {
            env: Env::new(),
            name: Name::new("x"),
            body: Term::TVar(Name::new("B")),
        },
    };
    let r = instantiate_tvar_value(&sigma, &Name::new("B"), &named_ty("C"));
    assert!(matches!(r, Value::Sigma { .. }));

    let pair = Value::Pair {
        fst: Box::new(Value::Neutral(Neutral::TVar(Name::new("B")))),
        snd: Box::new(Value::Literal(Literal::Int(1))),
    };
    let r = instantiate_tvar_value(&pair, &Name::new("B"), &named_ty("C"));
    assert!(matches!(r, Value::Pair { .. }));

    let u = Value::Universe(Level::zero());
    assert_eq!(
        instantiate_tvar_value(&u, &Name::new("B"), &named_ty("C")),
        u
    );
    assert_eq!(
        instantiate_tvar_value(
            &Value::Literal(Literal::Int(1)),
            &Name::new("B"),
            &named_ty("C")
        ),
        Value::Literal(Literal::Int(1))
    );

    let n = Neutral::TypeApp {
        fun: Box::new(Neutral::Var(Var::Named(Name::new("F")))),
        arg: Box::new(Value::Neutral(Neutral::TVar(Name::new("B")))),
    };
    let r = instantiate_tvar_value(&Value::Neutral(n), &Name::new("B"), &named_ty("C"));
    assert!(matches!(r, Value::Neutral(_)));

    let app = Neutral::App {
        fun: Box::new(Neutral::Var(Var::Named(Name::new("f")))),
        arg: Box::new(Value::Neutral(Neutral::TVar(Name::new("B")))),
    };
    let _ = instantiate_tvar_value(&Value::Neutral(app), &Name::new("B"), &named_ty("C"));

    let p1 = Neutral::Proj1(Box::new(Neutral::TVar(Name::new("B"))));
    let _ = instantiate_tvar_value(&Value::Neutral(p1), &Name::new("B"), &named_ty("C"));
    let p2 = Neutral::Proj2(Box::new(Neutral::TVar(Name::new("B"))));
    let _ = instantiate_tvar_value(&Value::Neutral(p2), &Name::new("B"), &named_ty("C"));
}

#[test]
fn unify_more_branches() {
    let mut ctx = MetaContext::new();

    let id1 = eval(&type_lam("A", Kind::Type, bvar(0)), &Env::new());
    let id2 = eval(&type_lam("A", Kind::Type, u0()), &Env::new());
    assert!(unify(&mut ctx, &id1, &id2).is_err());

    let id3 = eval(&type_lam("A", Kind::Type, bvar(0)), &Env::new());
    let id4 = eval(
        &type_lam("A", Kind::fun(Kind::Type, Kind::Type), bvar(0)),
        &Env::new(),
    );
    assert!(matches!(
        unify(&mut ctx, &id3, &id4).unwrap_err(),
        UnificationError::KindMismatch { .. }
    ));

    let f1 = forall("A", Kind::Type, named_ty("B"));
    let f2 = forall("A", Kind::Type, Value::Universe(Level::zero()));
    assert!(unify(&mut ctx, &f1, &f2).is_err());

    let f3 = forall("A", Kind::Type, named_ty("B"));
    let f4 = forall("A", Kind::fun(Kind::Type, Kind::Type), named_ty("B"));
    assert!(matches!(
        unify(&mut ctx, &f3, &f4).unwrap_err(),
        UnificationError::KindMismatch { .. }
    ));

    assert!(unify(&mut ctx, &Value::Literal(Literal::Int(1)), &named_ty("A")).is_err());
    assert!(
        unify(
            &mut ctx,
            &Value::Pair {
                fst: Box::new(Value::Literal(Literal::Int(1))),
                snd: Box::new(Value::Literal(Literal::Int(2))),
            },
            &Value::Universe(Level::zero())
        )
        .is_err()
    );

    let a1 = Value::Neutral(Neutral::App {
        fun: Box::new(Neutral::Var(Var::Named(Name::new("f")))),
        arg: Box::new(Value::Literal(Literal::Int(1))),
    });
    let a2 = a1.clone();
    assert!(unify(&mut ctx, &a1, &a2).is_ok());

    let t1 = Value::Neutral(Neutral::TypeApp {
        fun: Box::new(Neutral::Var(Var::Named(Name::new("F")))),
        arg: Box::new(Value::Universe(Level::zero())),
    });
    let t2 = t1.clone();
    assert!(unify(&mut ctx, &t1, &t2).is_ok());

    let p1 = Value::Neutral(Neutral::Proj1(Box::new(Neutral::Var(Var::Named(
        Name::new("p"),
    )))));
    let p2 = p1.clone();
    assert!(unify(&mut ctx, &p1, &p2).is_ok());

    assert!(
        unify(
            &mut ctx,
            &Value::Neutral(Neutral::Var(Var::Named("A".into()))),
            &Value::Neutral(Neutral::Var(Var::Named("B".into())))
        )
        .is_err()
    );
}

#[test]
fn occurs_check_branches() {
    let m = MetaVar(0);
    let m2 = MetaVar(1);

    let env = Env::new().extend(Value::Neutral(Neutral::Meta(m)));
    let cases = vec![
        Value::TypeLam {
            param_name: Name::new("A"),
            param_kind: Kind::Type,
            closure: Closure {
                env: env.clone(),
                name: Name::new("A"),
                body: Term::Meta(m),
            },
        },
        Value::Forall {
            param_name: Name::new("A"),
            param_kind: Kind::Type,
            body: Box::new(Value::Neutral(Neutral::Meta(m))),
        },
        Value::Pi {
            param_name: Name::new("x"),
            param: Box::new(Value::Literal(Literal::Int(1))),
            closure: Closure {
                env: env.clone(),
                name: Name::new("x"),
                body: Term::Meta(m),
            },
        },
        Value::Lambda {
            name: Name::new("x"),
            closure: Closure {
                env: env.clone(),
                name: Name::new("x"),
                body: Term::Meta(m),
            },
        },
        Value::Sigma {
            name: Name::new("x"),
            fst: Box::new(Value::Literal(Literal::Int(1))),
            closure: TypeClosure {
                env: env.clone(),
                name: Name::new("x"),
                body: Term::Meta(m),
            },
        },
        Value::Pair {
            fst: Box::new(Value::Neutral(Neutral::Meta(m))),
            snd: Box::new(Value::Literal(Literal::Int(1))),
        },
    ];
    for v in cases {
        let mut ctx = MetaContext::new();
        assert!(matches!(
            unify(&mut ctx, &Value::Neutral(Neutral::Meta(m)), &v).unwrap_err(),
            UnificationError::OccursCheck(0)
        ));
    }

    let mut ctx = MetaContext::new();
    ctx.subst.insert(m2, Value::Neutral(Neutral::Meta(m)));
    let v = Value::TypeLam {
        param_name: Name::new("A"),
        param_kind: Kind::Type,
        closure: Closure {
            env: Env::new(),
            name: Name::new("A"),
            body: Term::Meta(m2),
        },
    };
    assert!(matches!(
        unify(&mut ctx, &Value::Neutral(Neutral::Meta(m)), &v).unwrap_err(),
        UnificationError::OccursCheck(0)
    ));

    // TVar occurs through resolved substitution.
    let mut ctx = MetaContext::new();
    let body = Value::Forall {
        param_name: Name::new("B"),
        param_kind: Kind::Type,
        body: Box::new(Value::Neutral(Neutral::TVar(Name::new("A")))),
    };
    assert!(matches!(
        unify(
            &mut ctx,
            &Value::Neutral(Neutral::TVar(Name::new("A"))),
            &body
        )
        .unwrap_err(),
        UnificationError::TVarOccursCheck(_)
    ));
}

#[test]
fn synth_more_branches() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = TCtx::new();

    let s = synth(&mut ctx, &env, &tctx, &sigma("x", u0(), u0())).unwrap();
    assert!(matches!(s, Value::Universe(_)));

    let p = synth(
        &mut ctx,
        &env,
        &tctx,
        &pi("A", pi("B", u0(), u0()), bvar(0)),
    )
    .unwrap();
    assert!(matches!(p, Value::Universe(_)));

    let a = synth(&mut ctx, &env, &tctx, &alias("A", u0(), u0())).unwrap();
    assert!(matches!(a, Value::Universe(_)));

    let g = synth(
        &mut ctx,
        &env,
        &tctx,
        &gen_lam([("A", Kind::Type)], bvar(0)),
    )
    .unwrap();
    assert!(matches!(g, Value::Forall { .. }));

    assert_eq!(
        synth(&mut ctx, &env, &tctx, &lit_bool(true)).unwrap(),
        named_ty("Bool")
    );
    assert_eq!(
        synth(&mut ctx, &env, &tctx, &lit_string("hi")).unwrap(),
        named_ty("String")
    );

    assert!(matches!(
        synth_type(&mut ctx, &env, &tctx, &var("x")).unwrap_err(),
        TypeError::ScopeError(_)
    ));

    let bad = type_app(
        type_lam("A", Kind::fun(Kind::Type, Kind::Type), bvar(0)),
        u0(),
    );
    assert!(matches!(
        synth_type(&mut ctx, &env, &tctx, &bad).unwrap_err(),
        TypeError::KindMismatch { .. }
    ));

    let tctx = tctx.add_alias(Name::new("MyU"), u0(), Kind::Type);
    assert_eq!(
        synth_kind(&mut ctx, &env, &tctx, &tref("MyU")).unwrap(),
        Kind::Type
    );

    let tctx2 = tctx.extend_with_kind(
        Name::new("X"),
        Value::Neutral(Neutral::TVar(Name::new("X"))),
        Kind::Type,
    );
    assert_eq!(
        synth_kind(&mut ctx, &env, &tctx2, &var("X")).unwrap(),
        Kind::Type
    );
}

#[test]
fn synth_proj2_neutral() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = TCtx::new().extend(
        Name::new("p"),
        Value::Sigma {
            name: Name::new("x"),
            fst: Box::new(named_ty("Int")),
            closure: TypeClosure {
                env: Env::new(),
                name: Name::new("x"),
                body: var("Int"),
            },
        },
    );
    let ty = synth(&mut ctx, &env, &tctx, &proj2(bvar(0))).unwrap();
    assert_eq!(ty, named_ty("Int"));
}

#[test]
fn synth_proj2_pair() {
    let mut ctx = MetaContext::new();
    let env = Env::new().extend(Value::Lambda {
        name: Name::new("x"),
        closure: Closure {
            env: Env::new(),
            name: Name::new("x"),
            body: pair(lit_int(1), lit_int(2)),
        },
    });
    let tctx = TCtx::new().extend(
        Name::new("f"),
        val_pi(
            "x",
            named_ty("Int"),
            Env::new(),
            Name::new("x"),
            Term::Sigma {
                name: Name::new("y"),
                fst: Box::new(tvar("Int")),
                snd: Box::new(var("Int")),
            },
        ),
    );
    let e = app(bvar(0), lit_int(1));
    let ty = synth(&mut ctx, &env, &tctx, &proj2(e)).unwrap();
    assert_eq!(ty, named_ty("Int"));
}

#[test]
fn check_genlam_branches() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = TCtx::new();

    let id = gen_lam(
        [("A", Kind::fun(Kind::Type, Kind::Type))],
        lam("x", bvar(0)),
    );
    let ty = forall(
        "A",
        Kind::Type,
        val_pi("x", named_ty("A"), Env::new(), Name::new("x"), tvar("A")),
    );
    assert!(matches!(
        check(&mut ctx, &env, &tctx, &id, &ty).unwrap_err(),
        TypeError::KindMismatch { .. }
    ));

    let id2 = gen_lam([("B", Kind::Type)], lam("x", bvar(0)));
    let ty2 = forall(
        "A",
        Kind::Type,
        val_pi(
            "x",
            Value::Neutral(Neutral::TVar(Name::new("A"))),
            Env::new(),
            Name::new("x"),
            tvar("A"),
        ),
    );
    assert!(check(&mut ctx, &env, &tctx, &id2, &ty2).is_ok());

    let id3 = gen_lam([("A", Kind::Type), ("B", Kind::Type)], lam("x", bvar(0)));
    let ty3 = forall("A", Kind::Type, named_ty("Int"));
    assert!(matches!(
        check(&mut ctx, &env, &tctx, &id3, &ty3).unwrap_err(),
        TypeError::TypeMismatch { .. }
    ));
}

#[test]
fn convertible_error_propagation() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let m = ctx.fresh_meta();
    let tctx = TCtx::new().extend(Name::new("x"), Value::Neutral(Neutral::Meta(m)));
    let expected = Value::Neutral(Neutral::App {
        fun: Box::new(Neutral::Meta(m)),
        arg: Box::new(Value::Literal(Literal::Int(1))),
    });
    let err = check(&mut ctx, &env, &tctx, &bvar(0), &expected).unwrap_err();
    assert!(matches!(
        err,
        TypeError::Unification(UnificationError::OccursCheck(_))
    ));
}

#[test]
fn quote_more_branches() {
    let p = Value::Pair {
        fst: Box::new(Value::Literal(Literal::Int(1))),
        snd: Box::new(named_ty("A")),
    };
    assert_eq!(quote(&p, &[]), pair(lit_int(1), var("A")));

    let n = Neutral::TypeApp {
        fun: Box::new(Neutral::TVar(Name::new("F"))),
        arg: Box::new(Value::Universe(Level::zero())),
    };
    assert_eq!(quote(&Value::Neutral(n), &[]), type_app(tvar("F"), u0()));

    let napp = Neutral::App {
        fun: Box::new(Neutral::Var(Var::Named(Name::new("f")))),
        arg: Box::new(Value::Literal(Literal::Int(1))),
    };
    assert_eq!(quote(&Value::Neutral(napp), &[]), app(var("f"), lit_int(1)));

    let p1 = Neutral::Proj1(Box::new(Neutral::Var(Var::Named(Name::new("p")))));
    assert_eq!(quote(&Value::Neutral(p1), &[]), proj1(var("p")));
    let p2 = Neutral::Proj2(Box::new(Neutral::Var(Var::Named(Name::new("p")))));
    assert_eq!(quote(&Value::Neutral(p2), &[]), proj2(var("p")));

    let v = Value::Neutral(Neutral::Var(Var::Named(Name::new("x"))));
    assert_eq!(quote(&v, &[Name::new("x")]), bvar(0));
    assert_eq!(quote(&v, &[]), var("x"));

    let tv = Value::Neutral(Neutral::TVar(Name::new("A")));
    assert_eq!(quote(&tv, &[Name::new("A")]), bvar(0));
    assert_eq!(quote(&tv, &[]), tvar("A"));
}

#[test]
fn substitution_apply_more() {
    let mut sub = Substitution::new();
    sub.insert_tvar(
        Name::new("F"),
        eval(&type_lam("A", Kind::Type, bvar(0)), &Env::new()),
    );
    sub.insert_tvar(Name::new("A"), named_ty("A"));
    sub.insert(MetaVar(0), named_ty("B"));

    let type_lam = Value::TypeLam {
        param_name: Name::new("A"),
        param_kind: Kind::Type,
        closure: Closure {
            env: Env::new().extend(Value::Neutral(Neutral::TVar(Name::new("A")))),
            name: Name::new("A"),
            body: tvar("A"),
        },
    };
    let r = sub.apply_value(&type_lam);
    assert!(matches!(r, Value::TypeLam { .. }));

    let fa = Value::Forall {
        param_name: Name::new("A"),
        param_kind: Kind::Type,
        body: Box::new(Value::Neutral(Neutral::TVar(Name::new("A")))),
    };
    let r = sub.apply_value(&fa);
    assert!(matches!(r, Value::Forall { .. }));

    let pi = val_pi(
        "x",
        Value::Neutral(Neutral::TVar(Name::new("A"))),
        Env::new().extend(Value::Neutral(Neutral::TVar(Name::new("A")))),
        Name::new("x"),
        tvar("A"),
    );
    let r = sub.apply_value(&pi);
    assert!(matches!(r, Value::Pi { .. }));

    let lam = Value::Lambda {
        name: Name::new("x"),
        closure: Closure {
            env: Env::new().extend(Value::Neutral(Neutral::TVar(Name::new("A")))),
            name: Name::new("x"),
            body: tvar("A"),
        },
    };
    let r = sub.apply_value(&lam);
    assert!(matches!(r, Value::Lambda { .. }));

    let sigma = Value::Sigma {
        name: Name::new("x"),
        fst: Box::new(Value::Neutral(Neutral::TVar(Name::new("A")))),
        closure: TypeClosure {
            env: Env::new().extend(Value::Neutral(Neutral::TVar(Name::new("A")))),
            name: Name::new("x"),
            body: tvar("A"),
        },
    };
    let r = sub.apply_value(&sigma);
    assert!(matches!(r, Value::Sigma { .. }));

    let pair = Value::Pair {
        fst: Box::new(Value::Neutral(Neutral::TVar(Name::new("A")))),
        snd: Box::new(Value::Literal(Literal::Int(1))),
    };
    let r = sub.apply_value(&pair);
    assert!(matches!(r, Value::Pair { .. }));

    let lit = Value::Literal(Literal::Int(1));
    assert_eq!(sub.apply_value(&lit), lit);

    let type_app = Neutral::TypeApp {
        fun: Box::new(Neutral::TVar(Name::new("F"))),
        arg: Box::new(Value::Universe(Level::zero())),
    };
    let r = sub.apply_neutral(&type_app);
    assert_eq!(r, Value::Universe(Level::zero()));

    let app = Neutral::App {
        fun: Box::new(Neutral::TVar(Name::new("F"))),
        arg: Box::new(Value::Universe(Level::zero())),
    };
    let r = sub.apply_neutral(&app);
    assert_eq!(r, Value::Universe(Level::zero()));

    let pair_val = Value::Pair {
        fst: Box::new(named_ty("A")),
        snd: Box::new(named_ty("B")),
    };
    let mut sub2 = Substitution::new();
    sub2.insert_tvar(Name::new("A"), pair_val);
    let r = sub2.apply_neutral(&Neutral::Proj1(Box::new(Neutral::TVar(Name::new("A")))));
    assert_eq!(r, named_ty("A"));
    let r = sub2.apply_neutral(&Neutral::Proj2(Box::new(Neutral::TVar(Name::new("A")))));
    assert_eq!(r, named_ty("B"));

    let term = Term::Pair {
        fst: Box::new(Term::TVar(Name::new("A"))),
        snd: Box::new(Term::Pair {
            fst: Box::new(Term::TypeLam {
                name: Name::new("X"),
                param_kind: Kind::Type,
                body: Box::new(Term::TVar(Name::new("A"))),
            }),
            snd: Box::new(Term::TypeApp {
                fun: Box::new(Term::TVar(Name::new("F"))),
                arg: Box::new(u0()),
            }),
        }),
    };
    let _ = sub.apply_term(&term, &[]);

    let mut s1 = Substitution::new();
    s1.insert_tvar(
        Name::new("X"),
        Value::Neutral(Neutral::TVar(Name::new("A"))),
    );
    let mut s2 = Substitution::new();
    s2.insert_tvar(Name::new("A"), named_ty("B"));
    let composed = s1.compose(&s2);
    let r = composed.apply_value(&Value::Neutral(Neutral::TVar(Name::new("X"))));
    assert_eq!(r, named_ty("B"));
}

#[test]
fn level_and_core_smoke() {
    let a = Level::Max(Box::new(Level::zero()), Box::new(Level::zero().succ()));
    let b = Level::Max(Box::new(Level::zero().succ()), Box::new(Level::zero()));
    let m = Level::max(a, b);
    assert!(matches!(m, Level::Max(_, _)));
    assert!(!Level::zero().succ().succ().le(&Level::Max(
        Box::new(Level::zero()),
        Box::new(Level::zero().succ())
    )));

    let _ = Name::new("x");
    assert_eq!(Name::new("x").to_string(), "x");
    assert_eq!(Var::Named("y".into()).to_string(), "y");
    assert_eq!(Var::DeBruijn(5).to_string(), "#5");
    assert_eq!(
        format!("{}", Kind::fun(Kind::Type, Kind::Type)),
        "(Type -> Type)"
    );

    let env = Env::new();
    assert!(env.is_empty());
    let env = env.extend(Value::Literal(Literal::Int(1)));
    assert_eq!(env.len(), 1);
    assert_eq!(
        env.lookup(0).cloned(),
        Some(Value::Literal(Literal::Int(1)))
    );

    let tctx = TCtx::new();
    assert!(tctx.is_empty());
    let tctx = tctx
        .extend_with_kind(Name::new("X"), named_ty("X"), Kind::Type)
        .add_alias(Name::new("A"), u0(), Kind::Type);
    assert_eq!(tctx.lookup(0).cloned(), Some(named_ty("X")));
    assert_eq!(
        tctx.lookup_by_name(&Name::new("X")).cloned(),
        Some(named_ty("X"))
    );
    assert_eq!(tctx.lookup_kind(0).cloned(), Some(Kind::Type));
    assert_eq!(
        tctx.lookup_kind_by_name(&Name::new("X")).cloned(),
        Some(Kind::Type)
    );
    assert!(tctx.lookup_alias(&Name::new("A")).is_some());
    assert_eq!(tctx.names(), vec![Name::new("X")]);

    let c = Closure {
        env: Env::new().extend(Value::Literal(Literal::Int(1))),
        name: Name::new("x"),
        body: bvar(1),
    };
    assert_eq!(
        c.apply(Value::Literal(Literal::Int(2))),
        Value::Literal(Literal::Int(1))
    );
    let tc = TypeClosure {
        env: Env::new().extend(Value::Neutral(Neutral::TVar(Name::new("A")))),
        name: Name::new("x"),
        body: tvar("A"),
    };
    let _ = tc.apply(Value::Universe(Level::zero()));
}

#[test]
fn types_constructors() {
    let _ = alias("A", u0(), u0());
    let _ = type_lam("A", Kind::Type, bvar(0));
    let _ = gen_lam([("A", Kind::Type)], bvar(0));
    let _ = kfun(Kind::Type, Kind::Type);
    let _ = proj1(u0());
    let _ = proj2(u0());
    let _ = meta(0);
    let _ = hole(0);
    let _ = lit_bool(true);
    let _ = lit_string("x");
    let _ = val_pi("x", named_ty("A"), Env::new(), Name::new("x"), tvar("A"));
    let _ = forall("A", Kind::Type, named_ty("A"));
    let _ = r#let("x", lit_int(1), bvar(0));
}

#[test]
fn synth_kind_and_check_kind() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = TCtx::new();

    assert!(matches!(
        synth_kind(&mut ctx, &env, &tctx, &lam("x", bvar(0))).unwrap_err(),
        TypeError::CannotSynthesize(_)
    ));

    let id = type_lam("A", Kind::Type, bvar(0));
    assert!(matches!(
        check_kind(&mut ctx, &env, &tctx, &id, &Kind::Type).unwrap_err(),
        TypeError::KindMismatch { .. }
    ));
}

#[test]
fn eval_genlam_empty() {
    assert_eq!(
        eval(
            &gen_lam(Vec::<(Name, Kind)>::new(), lit_int(1)),
            &Env::new()
        ),
        Value::Literal(Literal::Int(1))
    );
}

#[test]
fn synth_expected_sigma() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = TCtx::new();
    assert!(matches!(
        synth(&mut ctx, &env, &tctx, &proj1(lit_int(1))).unwrap_err(),
        TypeError::ExpectedSigma(_)
    ));
    assert!(matches!(
        synth(&mut ctx, &env, &tctx, &proj2(lit_int(1))).unwrap_err(),
        TypeError::ExpectedSigma(_)
    ));
}

#[test]
fn synth_value_universe_branches() {
    let mut ctx = MetaContext::new();
    let env = Env::new();

    let tctx = TCtx::new().extend_with_kind(
        Name::new("S"),
        Value::Sigma {
            name: Name::new("x"),
            fst: Box::new(Value::Universe(Level::zero())),
            closure: TypeClosure {
                env: Env::new(),
                name: Name::new("x"),
                body: u0(),
            },
        },
        Kind::Type,
    );
    let _ = synth(&mut ctx, &env, &tctx, &pi("A", tvar("S"), u0())).unwrap();

    let tctx = TCtx::new().extend_with_kind(
        Name::new("F"),
        Value::Forall {
            param_name: Name::new("A"),
            param_kind: Kind::Type,
            body: Box::new(Value::Universe(Level::zero())),
        },
        Kind::Type,
    );
    let _ = synth(&mut ctx, &env, &tctx, &pi("A", tvar("F"), u0())).unwrap();

    let tctx = TCtx::new().extend_with_kind(
        Name::new("G"),
        Value::Forall {
            param_name: Name::new("A"),
            param_kind: Kind::fun(Kind::Type, Kind::Type),
            body: Box::new(Value::Universe(Level::zero())),
        },
        Kind::Type,
    );
    assert!(matches!(
        synth(&mut ctx, &env, &tctx, &pi("A", tvar("G"), u0())).unwrap_err(),
        TypeError::UniverseMismatch(_)
    ));

    let tctx = TCtx::new().extend_with_kind(
        Name::new("L"),
        Value::Lambda {
            name: Name::new("x"),
            closure: Closure {
                env: Env::new(),
                name: Name::new("x"),
                body: bvar(0),
            },
        },
        Kind::Type,
    );
    assert!(matches!(
        synth(&mut ctx, &env, &tctx, &pi("A", tvar("L"), u0())).unwrap_err(),
        TypeError::UniverseMismatch(_)
    ));
}

#[test]
fn synth_kind_missing_errors() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = TCtx::new();
    assert!(matches!(
        synth_kind(&mut ctx, &env, &tctx, &tref("Missing")).unwrap_err(),
        TypeError::AliasNotFound(_)
    ));
    assert!(matches!(
        synth_kind(&mut ctx, &env, &tctx, &var("Missing")).unwrap_err(),
        TypeError::UndefinedTypeVariable(_)
    ));
    let bad = type_app(
        type_lam("A", Kind::fun(Kind::Type, Kind::Type), bvar(0)),
        u0(),
    );
    assert!(matches!(
        synth_kind(&mut ctx, &env, &tctx, &bad).unwrap_err(),
        TypeError::KindMismatch { .. }
    ));
}

#[test]
fn instantiate_tvar_env_branch() {
    let ty_lam = Value::TypeLam {
        param_name: Name::new("A"),
        param_kind: Kind::Type,
        closure: Closure {
            env: Env::new().extend(Value::Neutral(Neutral::TVar(Name::new("B")))),
            name: Name::new("A"),
            body: bvar(1),
        },
    };
    let r = instantiate_tvar_value(&ty_lam, &Name::new("B"), &named_ty("C"));
    assert!(matches!(r, Value::TypeLam { .. }));
}

#[test]
fn checking_last_branches() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = TCtx::new();
    assert!(matches!(
        synth_kind(&mut ctx, &env, &tctx, &tvar("Missing")).unwrap_err(),
        TypeError::UndefinedTypeVariable(_)
    ));

    let tctx = tctx.add_alias(Name::new("A"), u0(), Kind::Type);
    assert_eq!(
        synth_kind(&mut ctx, &env, &tctx, &tref("A")).unwrap(),
        Kind::Type
    );

    let tctx = TCtx::new().extend(
        Name::new("x"),
        Value::Sigma {
            name: Name::new("y"),
            fst: Box::new(named_ty("Int")),
            closure: TypeClosure {
                env: Env::new(),
                name: Name::new("y"),
                body: u0(),
            },
        },
    );
    let env = Env::new().extend(Value::Literal(Literal::Int(1)));
    assert!(matches!(
        synth(&mut ctx, &env, &tctx, &proj2(bvar(0))).unwrap_err(),
        TypeError::ExpectedSigma(_)
    ));
}

#[test]
fn synth_tvar_success() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = TCtx::new().extend_with_kind(
        Name::new("U"),
        Value::Forall {
            param_name: Name::new("A"),
            param_kind: Kind::Type,
            body: Box::new(Value::Universe(Level::zero())),
        },
        Kind::Type,
    );
    let ty = synth(&mut ctx, &env, &tctx, &tvar("U")).unwrap();
    assert!(matches!(ty, Value::Universe(_)));
}

#[test]
fn comprehensive_coverage() {
    // Substitution used to exercise apply_term and apply_value.
    let mut sub = Substitution::new();
    sub.insert(
        MetaVar(0),
        Value::Pair {
            fst: Box::new(named_ty("A")),
            snd: Box::new(named_ty("B")),
        },
    );
    sub.insert_tvar(Name::new("A"), named_ty("X"));

    // 1. Construct one Term of each variant and run eval/quote/apply_term.
    let terms: [Term; 23] = [
        bvar(0),
        var("x"),
        u0(),
        type_lam("A", Kind::Type, bvar(0)),
        type_app(type_lam("A", Kind::Type, bvar(0)), u0()),
        tvar("A"),
        tref("A"),
        alias("A", u0(), tref("A")),
        gen_lam([("A", Kind::Type)], bvar(0)),
        gen_app(gen_lam([("A", Kind::Type)], tvar("A")), [u0()]),
        pi("A", u0(), bvar(0)),
        lam("x", bvar(0)),
        app(lam("x", bvar(0)), lit_int(7)),
        sigma("A", u0(), bvar(0)),
        pair(lit_int(1), lit_int(2)),
        proj1(pair(lit_int(1), lit_int(2))),
        proj2(pair(lit_int(1), lit_int(2))),
        r#let("x", lit_int(5), bvar(0)),
        meta(0),
        hole(0),
        lit_bool(true),
        lit_int(1),
        lit_string("hello"),
    ];
    for t in terms.iter() {
        let v = eval(t, &Env::new());
        let _q = quote(&v, &[]);
        let _applied = sub.apply_term(t, &[]);
    }

    // 2. Type-level and checking pipelines for individual Term variants.
    let mut ctx = MetaContext::new();

    // Universe
    assert!(synth_type(&mut ctx, &Env::new(), &TCtx::new(), &u0()).is_ok());
    assert!(synth(&mut ctx, &Env::new(), &TCtx::new(), &u0()).is_ok());
    assert!(
        check(
            &mut ctx,
            &Env::new(),
            &TCtx::new(),
            &u0(),
            &Value::Universe(Level::zero().succ())
        )
        .is_ok()
    );

    // Variable (DeBruijn)
    let env = Env::new().extend(Value::Universe(Level::zero()));
    assert!(synth_type(&mut ctx, &env, &TCtx::new(), &bvar(0)).is_ok());
    let tctx_x = TCtx::new().extend(Name::new("x"), named_ty("Int"));
    assert!(synth(&mut ctx, &Env::new(), &tctx_x, &bvar(0)).is_ok());
    assert!(synth(&mut ctx, &Env::new(), &TCtx::new(), &var("x")).is_err());

    // Type variables and aliases
    let tctx_tvar = TCtx::new()
        .extend_with_kind(Name::new("Int"), named_ty("Int"), Kind::Type)
        .extend_with_kind(Name::new("A"), named_ty("A"), Kind::Type);
    assert!(synth_type(&mut ctx, &Env::new(), &tctx_tvar, &tvar("A")).is_ok());
    assert!(synth(&mut ctx, &Env::new(), &tctx_tvar, &tvar("A")).is_ok());
    let ty = synth_type(&mut ctx, &Env::new(), &tctx_tvar, &tvar("Int")).unwrap();
    assert!(check(&mut ctx, &Env::new(), &tctx_tvar, &lit_int(5), &ty).is_ok());

    let tctx_alias = TCtx::new()
        .extend_with_kind(Name::new("Int"), named_ty("Int"), Kind::Type)
        .add_alias(Name::new("MyInt"), tvar("Int"), Kind::Type);
    assert!(synth_type(&mut ctx, &Env::new(), &tctx_alias, &tref("MyInt")).is_ok());
    assert!(synth(&mut ctx, &Env::new(), &tctx_alias, &tref("MyInt")).is_ok());
    let ty = synth_type(&mut ctx, &Env::new(), &tctx_alias, &tref("MyInt")).unwrap();
    assert!(check(&mut ctx, &Env::new(), &tctx_alias, &lit_int(5), &ty).is_ok());
    assert!(
        synth(
            &mut ctx,
            &Env::new(),
            &tctx_alias,
            &alias("MyInt", tvar("Int"), tref("MyInt"))
        )
        .is_ok()
    );

    // Type-level lambda and application
    let id_t = type_lam("A", Kind::Type, bvar(0));
    assert!(synth_type(&mut ctx, &Env::new(), &TCtx::new(), &id_t).is_ok());
    assert!(synth(&mut ctx, &Env::new(), &TCtx::new(), &id_t).is_err());
    assert!(synth_type(&mut ctx, &Env::new(), &TCtx::new(), &type_app(id_t, u0())).is_ok());
    assert!(
        synth(
            &mut ctx,
            &Env::new(),
            &TCtx::new(),
            &type_app(type_lam("A", Kind::Type, bvar(0)), u0())
        )
        .is_ok()
    );

    // Pi / Lambda / App
    let pi_t = pi("A", u0(), bvar(0));
    assert!(synth_type(&mut ctx, &Env::new(), &TCtx::new(), &pi_t).is_ok());
    assert!(synth(&mut ctx, &Env::new(), &TCtx::new(), &pi_t).is_ok());
    assert!(synth(&mut ctx, &Env::new(), &TCtx::new(), &lam("x", bvar(0))).is_err());
    let pi_ty = Value::Pi {
        param_name: Name::new("x"),
        param: Box::new(named_ty("Int")),
        closure: Closure {
            env: Env::new(),
            name: Name::new("x"),
            body: var("Int"),
        },
    };
    assert!(
        check(
            &mut ctx,
            &Env::new(),
            &TCtx::new(),
            &lam("x", bvar(0)),
            &pi_ty
        )
        .is_ok()
    );
    let fty = val_pi("x", named_ty("Int"), Env::new(), Name::new("x"), var("Int"));
    let tctx_f = TCtx::new().extend(Name::new("f"), fty);
    assert_eq!(
        synth(&mut ctx, &Env::new(), &tctx_f, &app(bvar(0), lit_int(5))).unwrap(),
        named_ty("Int")
    );

    // Sigma / Pair / Proj
    let sigma_t = sigma("A", u0(), bvar(0));
    assert!(synth_type(&mut ctx, &Env::new(), &TCtx::new(), &sigma_t).is_ok());
    assert!(synth(&mut ctx, &Env::new(), &TCtx::new(), &sigma_t).is_ok());
    assert!(
        synth(
            &mut ctx,
            &Env::new(),
            &TCtx::new(),
            &pair(lit_int(1), lit_int(2))
        )
        .is_err()
    );
    let sigma_ty = Value::Sigma {
        name: Name::new("x"),
        fst: Box::new(named_ty("Int")),
        closure: TypeClosure {
            env: Env::new(),
            name: Name::new("x"),
            body: var("Int"),
        },
    };
    assert!(
        check(
            &mut ctx,
            &Env::new(),
            &TCtx::new(),
            &pair(lit_int(1), lit_int(2)),
            &sigma_ty
        )
        .is_ok()
    );
    let tctx_p = TCtx::new().extend(Name::new("p"), sigma_ty);
    assert!(synth(&mut ctx, &Env::new(), &tctx_p, &proj1(bvar(0))).is_ok());
    assert!(synth(&mut ctx, &Env::new(), &tctx_p, &proj2(bvar(0))).is_ok());

    // Let
    assert!(
        synth(
            &mut ctx,
            &Env::new(),
            &TCtx::new(),
            &r#let("x", lit_int(5), bvar(0))
        )
        .is_ok()
    );

    // GenLam / GenApp
    let gl = gen_lam([("A", Kind::Type)], bvar(0));
    assert!(synth(&mut ctx, &Env::new(), &TCtx::new(), &gl).is_ok());
    let ga = gen_app(gen_lam([("A", Kind::Type)], bvar(0)), [u0()]);
    assert!(synth(&mut ctx, &Env::new(), &TCtx::new(), &ga).is_ok());

    // Meta / Hole
    assert!(synth(&mut ctx, &Env::new(), &TCtx::new(), &meta(0)).is_err());
    assert!(synth(&mut ctx, &Env::new(), &TCtx::new(), &hole(0)).is_err());

    // Literals
    assert_eq!(
        synth(&mut ctx, &Env::new(), &TCtx::new(), &lit_bool(true)).unwrap(),
        named_ty("Bool")
    );
    assert_eq!(
        synth(&mut ctx, &Env::new(), &TCtx::new(), &lit_int(1)).unwrap(),
        named_ty("Int")
    );
    assert_eq!(
        synth(&mut ctx, &Env::new(), &TCtx::new(), &lit_string("s")).unwrap(),
        named_ty("String")
    );

    // 3. Construct one Value of each variant and run unify/apply_value.
    let values: [Value; 17] = [
        Value::Neutral(Neutral::Var(Var::Named("x".into()))),
        Value::Neutral(Neutral::Var(Var::DeBruijn(0))),
        Value::Neutral(Neutral::TypeApp {
            fun: Box::new(Neutral::TVar(Name::new("A"))),
            arg: Box::new(Value::Universe(Level::zero())),
        }),
        Value::Neutral(Neutral::TVar(Name::new("A"))),
        Value::Neutral(Neutral::App {
            fun: Box::new(Neutral::Var(Var::Named("f".into()))),
            arg: Box::new(Value::Literal(Literal::Int(1))),
        }),
        Value::Neutral(Neutral::Proj1(Box::new(Neutral::Meta(MetaVar(0))))),
        Value::Neutral(Neutral::Proj2(Box::new(Neutral::Meta(MetaVar(0))))),
        Value::Neutral(Neutral::Meta(MetaVar(0))),
        Value::Neutral(Neutral::Hole(HoleId(0))),
        Value::Universe(Level::zero()),
        Value::TypeLam {
            param_name: Name::new("A"),
            param_kind: Kind::Type,
            closure: Closure {
                env: Env::new().extend(Value::Neutral(Neutral::Meta(MetaVar(0)))),
                name: Name::new("A"),
                body: bvar(0),
            },
        },
        Value::Forall {
            param_name: Name::new("B"),
            param_kind: Kind::Type,
            body: Box::new(Value::Neutral(Neutral::TVar(Name::new("A")))),
        },
        Value::Pi {
            param_name: Name::new("x"),
            param: Box::new(Value::Neutral(Neutral::TVar(Name::new("A")))),
            closure: Closure {
                env: Env::new().extend(Value::Neutral(Neutral::TVar(Name::new("A")))),
                name: Name::new("x"),
                body: bvar(0),
            },
        },
        Value::Lambda {
            name: Name::new("x"),
            closure: Closure {
                env: Env::new().extend(Value::Neutral(Neutral::TVar(Name::new("A")))),
                name: Name::new("x"),
                body: bvar(0),
            },
        },
        Value::Sigma {
            name: Name::new("x"),
            fst: Box::new(Value::Neutral(Neutral::TVar(Name::new("A")))),
            closure: TypeClosure {
                env: Env::new().extend(Value::Neutral(Neutral::TVar(Name::new("A")))),
                name: Name::new("x"),
                body: bvar(0),
            },
        },
        Value::Pair {
            fst: Box::new(Value::Neutral(Neutral::TVar(Name::new("A")))),
            snd: Box::new(Value::Literal(Literal::Int(2))),
        },
        Value::Literal(Literal::String("s".into())),
    ];
    for v in values.iter() {
        let _applied = sub.apply_value(v);
    }
    let flex = Value::Neutral(Neutral::Meta(MetaVar(1000)));
    for v in values.iter() {
        let mut uctx = MetaContext::new();
        assert!(unify(&mut uctx, v, v).is_ok());
        assert!(unify(&mut uctx, &flex, v).is_ok());
    }

    // 4. Compose a substitution that touches every value variant.
    let mut s1 = Substitution::new();
    s1.insert(MetaVar(0), Value::Universe(Level::zero()));
    s1.insert_tvar(
        Name::new("A"),
        Value::TypeLam {
            param_name: Name::new("A"),
            param_kind: Kind::Type,
            closure: Closure {
                env: Env::new(),
                name: Name::new("A"),
                body: bvar(0),
            },
        },
    );
    let mut s2 = Substitution::new();
    for (i, v) in values.iter().enumerate() {
        s2.insert(MetaVar(i as u32), v.clone());
    }
    s2.insert_tvar(Name::new("B"), named_ty("Y"));
    let composed = s1.compose(&s2);
    assert!(matches!(
        composed.apply_value(&Value::Neutral(Neutral::Meta(MetaVar(0)))),
        Value::Universe(_)
    ));
    assert!(matches!(
        composed.apply_value(&Value::Neutral(Neutral::TVar(Name::new("A")))),
        Value::TypeLam { .. }
    ));
}

#[test]
fn primitive_membership_inhabited_by_is_evidence() {
    let proof = is(lit_int(4), collection("type.number"));
    assert_eq!(eval(&proof, &Env::new()), Value::IsEvidence);
}

#[test]
fn is_int_number_well_typed_and_evaluates_to_proof() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = prelude_tctx();
    let ty = synth(&mut ctx, &env, &tctx, &is(lit_int(4), collection("type.number")));
    assert!(ty.is_ok());
    assert_eq!(eval(&is(lit_int(4), collection("type.number")), &env), Value::IsEvidence);
}

#[test]
fn is_string_number_is_type_error() {
    let mut ctx = MetaContext::new();
    let env = Env::new();
    let tctx = prelude_tctx();
    let err = synth(
        &mut ctx,
        &env,
        &tctx,
        &is(lit_string("x"), collection("type.number")),
    )
    .unwrap_err();
    assert!(matches!(err, TypeError::TypeMismatch { .. }));
}

#[test]
fn refl_checks_against_equal_ints() {
    let mut ctx = MetaContext::new();
    let ty = eval(&eq(lit_int(4), lit_int(4)), &Env::new());
    assert!(check(&mut ctx, &Env::new(), &TCtx::new(), &refl(), &ty).is_ok());
}

#[test]
fn typed_let_number_works() {
    let mut ctx = MetaContext::new();
    let t = typed_let("n", lit_int(4), collection("type.number"), bvar(0));
    let ty = synth(&mut ctx, &Env::new(), &prelude_tctx(), &t).unwrap();
    assert_eq!(ty, Value::Collection(Name::new("type.number")));
}

#[test]
fn synth_prelude_collection() {
    let mut ctx = MetaContext::new();
    let ty = synth(&mut ctx, &Env::new(), &prelude_tctx(), &var("type.number")).unwrap();
    assert_eq!(ty, Value::Collection(Name::new("type.number")));
}
