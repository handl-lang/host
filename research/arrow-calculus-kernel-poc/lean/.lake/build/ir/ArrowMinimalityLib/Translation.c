// Lean compiler output
// Module: ArrowMinimalityLib.Translation
// Imports: public import Init public meta import Init
#include <lean/lean.h>
#if defined(__clang__)
#pragma clang diagnostic ignored "-Wunused-parameter"
#pragma clang diagnostic ignored "-Wunused-label"
#elif defined(__GNUC__) && !defined(__CLANG__)
#pragma GCC diagnostic ignored "-Wunused-parameter"
#pragma GCC diagnostic ignored "-Wunused-label"
#pragma GCC diagnostic ignored "-Wunused-but-set-variable"
#endif
#ifdef __cplusplus
extern "C" {
#endif
uint8_t lean_nat_dec_lt(lean_object*, lean_object*);
lean_object* lean_nat_add(lean_object*, lean_object*);
uint8_t lean_nat_dec_eq(lean_object*, lean_object*);
lean_object* lean_nat_sub(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorIdx(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorIdx___boxed(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim___boxed(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_var_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_var_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_lam_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_lam_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_app_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_app_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_pair_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_pair_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_fst_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_fst_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_snd_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_snd_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_inl_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_inl_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_inr_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_inr_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_case_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_case_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_fix_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_fix_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_unit_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_unit_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_shift(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_shift___boxed(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_substAux(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_substAux___boxed(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_subst0(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_lamSize(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_lamSize___boxed(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_Translation_0__TranslationAsymmetry_LamTerm_shift_match__1_splitter___redArg(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_Translation_0__TranslationAsymmetry_LamTerm_shift_match__1_splitter(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_ctorIdx(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_ctorIdx___boxed(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_ctorElim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_ctorElim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_ctorElim___boxed(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_arr_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_arr_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_comp_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_comp_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_first_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_first_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_left_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_left_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_loop_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_loop_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_arrowSize(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_arrowSize___boxed(lean_object*);
static const lean_ctor_object lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 0}, .m_objs = {((lean_object*)(((size_t)(0) << 1) | 1))}};
static const lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__0 = (const lean_object*)&lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__0_value;
static const lean_ctor_object lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__1_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 4}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__0_value)}};
static const lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__1 = (const lean_object*)&lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__1_value;
static const lean_ctor_object lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__2_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 5}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__0_value)}};
static const lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__2 = (const lean_object*)&lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__2_value;
static const lean_ctor_object lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__3_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 7}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__0_value)}};
static const lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__3 = (const lean_object*)&lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__3_value;
static const lean_ctor_object lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__4_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 0}, .m_objs = {((lean_object*)(((size_t)(1) << 1) | 1))}};
static const lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__4 = (const lean_object*)&lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__4_value;
static const lean_ctor_object lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__5_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 3}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__4_value),((lean_object*)&lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__0_value)}};
static const lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__5 = (const lean_object*)&lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__5_value;
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_Translation_0__TranslationAsymmetry_ArrowProg_allArrClosed_match__1_splitter___redArg(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_Translation_0__TranslationAsymmetry_ArrowProg_allArrClosed_match__1_splitter(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LoopGap_loopExtLHS(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LoopGap_loopExtRHS(lean_object*, lean_object*);
static const lean_ctor_object lp_arrow__minimality__lib_TranslationAsymmetry_LoopGap_interp___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 1}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__0_value)}};
static const lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LoopGap_interp___closed__0 = (const lean_object*)&lp_arrow__minimality__lib_TranslationAsymmetry_LoopGap_interp___closed__0_value;
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LoopGap_interp(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_ctorIdx(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_ctorIdx___boxed(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_ctorElim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_ctorElim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_ctorElim___boxed(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_arr_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_arr_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_comp_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_comp_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_first_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_first_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_left_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_left_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_loop_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_loop_elim(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_app_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_app_elim(lean_object*, lean_object*, lean_object*, lean_object*);
static const lean_ctor_object lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_S___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 0}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__0_value)}};
static const lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_S___closed__0 = (const lean_object*)&lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_S___closed__0_value;
static const lean_ctor_object lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_S___closed__1_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 0}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_TranslationAsymmetry_LoopGap_interp___closed__0_value)}};
static const lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_S___closed__1 = (const lean_object*)&lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_S___closed__1_value;
static const lean_ctor_object lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_S___closed__2_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 3}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__0_value),((lean_object*)&lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__0_value)}};
static const lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_S___closed__2 = (const lean_object*)&lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_S___closed__2_value;
static const lean_ctor_object lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_S___closed__3_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 0}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_S___closed__2_value)}};
static const lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_S___closed__3 = (const lean_object*)&lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_S___closed__3_value;
static const lean_ctor_object lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_S___closed__4_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 1}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_S___closed__3_value),((lean_object*)(((size_t)(5) << 1) | 1))}};
static const lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_S___closed__4 = (const lean_object*)&lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_S___closed__4_value;
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_S(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorIdx(lean_object* v_x_1_){
_start:
{
switch(lean_obj_tag(v_x_1_))
{
case 0:
{
lean_object* v___x_2_; 
v___x_2_ = lean_unsigned_to_nat(0u);
return v___x_2_;
}
case 1:
{
lean_object* v___x_3_; 
v___x_3_ = lean_unsigned_to_nat(1u);
return v___x_3_;
}
case 2:
{
lean_object* v___x_4_; 
v___x_4_ = lean_unsigned_to_nat(2u);
return v___x_4_;
}
case 3:
{
lean_object* v___x_5_; 
v___x_5_ = lean_unsigned_to_nat(3u);
return v___x_5_;
}
case 4:
{
lean_object* v___x_6_; 
v___x_6_ = lean_unsigned_to_nat(4u);
return v___x_6_;
}
case 5:
{
lean_object* v___x_7_; 
v___x_7_ = lean_unsigned_to_nat(5u);
return v___x_7_;
}
case 6:
{
lean_object* v___x_8_; 
v___x_8_ = lean_unsigned_to_nat(6u);
return v___x_8_;
}
case 7:
{
lean_object* v___x_9_; 
v___x_9_ = lean_unsigned_to_nat(7u);
return v___x_9_;
}
case 8:
{
lean_object* v___x_10_; 
v___x_10_ = lean_unsigned_to_nat(8u);
return v___x_10_;
}
case 9:
{
lean_object* v___x_11_; 
v___x_11_ = lean_unsigned_to_nat(9u);
return v___x_11_;
}
default: 
{
lean_object* v___x_12_; 
v___x_12_ = lean_unsigned_to_nat(10u);
return v___x_12_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorIdx___boxed(lean_object* v_x_13_){
_start:
{
lean_object* v_res_14_; 
v_res_14_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorIdx(v_x_13_);
lean_dec(v_x_13_);
return v_res_14_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim___redArg(lean_object* v_t_15_, lean_object* v_k_16_){
_start:
{
switch(lean_obj_tag(v_t_15_))
{
case 2:
{
lean_object* v_a_17_; lean_object* v_a_18_; lean_object* v___x_19_; 
v_a_17_ = lean_ctor_get(v_t_15_, 0);
lean_inc(v_a_17_);
v_a_18_ = lean_ctor_get(v_t_15_, 1);
lean_inc(v_a_18_);
lean_dec_ref_known(v_t_15_, 2);
v___x_19_ = lean_apply_2(v_k_16_, v_a_17_, v_a_18_);
return v___x_19_;
}
case 3:
{
lean_object* v_a_20_; lean_object* v_a_21_; lean_object* v___x_22_; 
v_a_20_ = lean_ctor_get(v_t_15_, 0);
lean_inc(v_a_20_);
v_a_21_ = lean_ctor_get(v_t_15_, 1);
lean_inc(v_a_21_);
lean_dec_ref_known(v_t_15_, 2);
v___x_22_ = lean_apply_2(v_k_16_, v_a_20_, v_a_21_);
return v___x_22_;
}
case 8:
{
lean_object* v_a_23_; lean_object* v_a_24_; lean_object* v_a_25_; lean_object* v___x_26_; 
v_a_23_ = lean_ctor_get(v_t_15_, 0);
lean_inc(v_a_23_);
v_a_24_ = lean_ctor_get(v_t_15_, 1);
lean_inc(v_a_24_);
v_a_25_ = lean_ctor_get(v_t_15_, 2);
lean_inc(v_a_25_);
lean_dec_ref_known(v_t_15_, 3);
v___x_26_ = lean_apply_3(v_k_16_, v_a_23_, v_a_24_, v_a_25_);
return v___x_26_;
}
case 10:
{
return v_k_16_;
}
default: 
{
lean_object* v_a_27_; lean_object* v___x_28_; 
v_a_27_ = lean_ctor_get(v_t_15_, 0);
lean_inc(v_a_27_);
lean_dec(v_t_15_);
v___x_28_ = lean_apply_1(v_k_16_, v_a_27_);
return v___x_28_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim(lean_object* v_motive_29_, lean_object* v_ctorIdx_30_, lean_object* v_t_31_, lean_object* v_h_32_, lean_object* v_k_33_){
_start:
{
lean_object* v___x_34_; 
v___x_34_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim___redArg(v_t_31_, v_k_33_);
return v___x_34_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim___boxed(lean_object* v_motive_35_, lean_object* v_ctorIdx_36_, lean_object* v_t_37_, lean_object* v_h_38_, lean_object* v_k_39_){
_start:
{
lean_object* v_res_40_; 
v_res_40_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim(v_motive_35_, v_ctorIdx_36_, v_t_37_, v_h_38_, v_k_39_);
lean_dec(v_ctorIdx_36_);
return v_res_40_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_var_elim___redArg(lean_object* v_t_41_, lean_object* v_var_42_){
_start:
{
lean_object* v___x_43_; 
v___x_43_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim___redArg(v_t_41_, v_var_42_);
return v___x_43_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_var_elim(lean_object* v_motive_44_, lean_object* v_t_45_, lean_object* v_h_46_, lean_object* v_var_47_){
_start:
{
lean_object* v___x_48_; 
v___x_48_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim___redArg(v_t_45_, v_var_47_);
return v___x_48_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_lam_elim___redArg(lean_object* v_t_49_, lean_object* v_lam_50_){
_start:
{
lean_object* v___x_51_; 
v___x_51_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim___redArg(v_t_49_, v_lam_50_);
return v___x_51_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_lam_elim(lean_object* v_motive_52_, lean_object* v_t_53_, lean_object* v_h_54_, lean_object* v_lam_55_){
_start:
{
lean_object* v___x_56_; 
v___x_56_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim___redArg(v_t_53_, v_lam_55_);
return v___x_56_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_app_elim___redArg(lean_object* v_t_57_, lean_object* v_app_58_){
_start:
{
lean_object* v___x_59_; 
v___x_59_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim___redArg(v_t_57_, v_app_58_);
return v___x_59_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_app_elim(lean_object* v_motive_60_, lean_object* v_t_61_, lean_object* v_h_62_, lean_object* v_app_63_){
_start:
{
lean_object* v___x_64_; 
v___x_64_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim___redArg(v_t_61_, v_app_63_);
return v___x_64_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_pair_elim___redArg(lean_object* v_t_65_, lean_object* v_pair_66_){
_start:
{
lean_object* v___x_67_; 
v___x_67_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim___redArg(v_t_65_, v_pair_66_);
return v___x_67_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_pair_elim(lean_object* v_motive_68_, lean_object* v_t_69_, lean_object* v_h_70_, lean_object* v_pair_71_){
_start:
{
lean_object* v___x_72_; 
v___x_72_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim___redArg(v_t_69_, v_pair_71_);
return v___x_72_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_fst_elim___redArg(lean_object* v_t_73_, lean_object* v_fst_74_){
_start:
{
lean_object* v___x_75_; 
v___x_75_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim___redArg(v_t_73_, v_fst_74_);
return v___x_75_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_fst_elim(lean_object* v_motive_76_, lean_object* v_t_77_, lean_object* v_h_78_, lean_object* v_fst_79_){
_start:
{
lean_object* v___x_80_; 
v___x_80_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim___redArg(v_t_77_, v_fst_79_);
return v___x_80_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_snd_elim___redArg(lean_object* v_t_81_, lean_object* v_snd_82_){
_start:
{
lean_object* v___x_83_; 
v___x_83_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim___redArg(v_t_81_, v_snd_82_);
return v___x_83_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_snd_elim(lean_object* v_motive_84_, lean_object* v_t_85_, lean_object* v_h_86_, lean_object* v_snd_87_){
_start:
{
lean_object* v___x_88_; 
v___x_88_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim___redArg(v_t_85_, v_snd_87_);
return v___x_88_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_inl_elim___redArg(lean_object* v_t_89_, lean_object* v_inl_90_){
_start:
{
lean_object* v___x_91_; 
v___x_91_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim___redArg(v_t_89_, v_inl_90_);
return v___x_91_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_inl_elim(lean_object* v_motive_92_, lean_object* v_t_93_, lean_object* v_h_94_, lean_object* v_inl_95_){
_start:
{
lean_object* v___x_96_; 
v___x_96_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim___redArg(v_t_93_, v_inl_95_);
return v___x_96_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_inr_elim___redArg(lean_object* v_t_97_, lean_object* v_inr_98_){
_start:
{
lean_object* v___x_99_; 
v___x_99_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim___redArg(v_t_97_, v_inr_98_);
return v___x_99_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_inr_elim(lean_object* v_motive_100_, lean_object* v_t_101_, lean_object* v_h_102_, lean_object* v_inr_103_){
_start:
{
lean_object* v___x_104_; 
v___x_104_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim___redArg(v_t_101_, v_inr_103_);
return v___x_104_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_case_elim___redArg(lean_object* v_t_105_, lean_object* v_case_106_){
_start:
{
lean_object* v___x_107_; 
v___x_107_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim___redArg(v_t_105_, v_case_106_);
return v___x_107_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_case_elim(lean_object* v_motive_108_, lean_object* v_t_109_, lean_object* v_h_110_, lean_object* v_case_111_){
_start:
{
lean_object* v___x_112_; 
v___x_112_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim___redArg(v_t_109_, v_case_111_);
return v___x_112_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_fix_elim___redArg(lean_object* v_t_113_, lean_object* v_fix_114_){
_start:
{
lean_object* v___x_115_; 
v___x_115_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim___redArg(v_t_113_, v_fix_114_);
return v___x_115_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_fix_elim(lean_object* v_motive_116_, lean_object* v_t_117_, lean_object* v_h_118_, lean_object* v_fix_119_){
_start:
{
lean_object* v___x_120_; 
v___x_120_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim___redArg(v_t_117_, v_fix_119_);
return v___x_120_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_unit_elim___redArg(lean_object* v_t_121_, lean_object* v_unit_122_){
_start:
{
lean_object* v___x_123_; 
v___x_123_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim___redArg(v_t_121_, v_unit_122_);
return v___x_123_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_unit_elim(lean_object* v_motive_124_, lean_object* v_t_125_, lean_object* v_h_126_, lean_object* v_unit_127_){
_start:
{
lean_object* v___x_128_; 
v___x_128_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_ctorElim___redArg(v_t_125_, v_unit_127_);
return v___x_128_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_shift(lean_object* v_d_129_, lean_object* v_c_130_, lean_object* v_x_131_){
_start:
{
switch(lean_obj_tag(v_x_131_))
{
case 0:
{
lean_object* v_a_132_; uint8_t v___x_133_; 
v_a_132_ = lean_ctor_get(v_x_131_, 0);
v___x_133_ = lean_nat_dec_lt(v_a_132_, v_c_130_);
if (v___x_133_ == 0)
{
lean_object* v___x_135_; uint8_t v_isShared_136_; uint8_t v_isSharedCheck_141_; 
lean_inc(v_a_132_);
v_isSharedCheck_141_ = !lean_is_exclusive(v_x_131_);
if (v_isSharedCheck_141_ == 0)
{
lean_object* v_unused_142_; 
v_unused_142_ = lean_ctor_get(v_x_131_, 0);
lean_dec(v_unused_142_);
v___x_135_ = v_x_131_;
v_isShared_136_ = v_isSharedCheck_141_;
goto v_resetjp_134_;
}
else
{
lean_dec(v_x_131_);
v___x_135_ = lean_box(0);
v_isShared_136_ = v_isSharedCheck_141_;
goto v_resetjp_134_;
}
v_resetjp_134_:
{
lean_object* v___x_137_; lean_object* v___x_139_; 
v___x_137_ = lean_nat_add(v_a_132_, v_d_129_);
lean_dec(v_a_132_);
if (v_isShared_136_ == 0)
{
lean_ctor_set(v___x_135_, 0, v___x_137_);
v___x_139_ = v___x_135_;
goto v_reusejp_138_;
}
else
{
lean_object* v_reuseFailAlloc_140_; 
v_reuseFailAlloc_140_ = lean_alloc_ctor(0, 1, 0);
lean_ctor_set(v_reuseFailAlloc_140_, 0, v___x_137_);
v___x_139_ = v_reuseFailAlloc_140_;
goto v_reusejp_138_;
}
v_reusejp_138_:
{
return v___x_139_;
}
}
}
else
{
return v_x_131_;
}
}
case 1:
{
lean_object* v_a_143_; lean_object* v___x_145_; uint8_t v_isShared_146_; uint8_t v_isSharedCheck_153_; 
v_a_143_ = lean_ctor_get(v_x_131_, 0);
v_isSharedCheck_153_ = !lean_is_exclusive(v_x_131_);
if (v_isSharedCheck_153_ == 0)
{
v___x_145_ = v_x_131_;
v_isShared_146_ = v_isSharedCheck_153_;
goto v_resetjp_144_;
}
else
{
lean_inc(v_a_143_);
lean_dec(v_x_131_);
v___x_145_ = lean_box(0);
v_isShared_146_ = v_isSharedCheck_153_;
goto v_resetjp_144_;
}
v_resetjp_144_:
{
lean_object* v___x_147_; lean_object* v___x_148_; lean_object* v___x_149_; lean_object* v___x_151_; 
v___x_147_ = lean_unsigned_to_nat(1u);
v___x_148_ = lean_nat_add(v_c_130_, v___x_147_);
v___x_149_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_shift(v_d_129_, v___x_148_, v_a_143_);
lean_dec(v___x_148_);
if (v_isShared_146_ == 0)
{
lean_ctor_set(v___x_145_, 0, v___x_149_);
v___x_151_ = v___x_145_;
goto v_reusejp_150_;
}
else
{
lean_object* v_reuseFailAlloc_152_; 
v_reuseFailAlloc_152_ = lean_alloc_ctor(1, 1, 0);
lean_ctor_set(v_reuseFailAlloc_152_, 0, v___x_149_);
v___x_151_ = v_reuseFailAlloc_152_;
goto v_reusejp_150_;
}
v_reusejp_150_:
{
return v___x_151_;
}
}
}
case 2:
{
lean_object* v_a_154_; lean_object* v_a_155_; lean_object* v___x_157_; uint8_t v_isShared_158_; uint8_t v_isSharedCheck_164_; 
v_a_154_ = lean_ctor_get(v_x_131_, 0);
v_a_155_ = lean_ctor_get(v_x_131_, 1);
v_isSharedCheck_164_ = !lean_is_exclusive(v_x_131_);
if (v_isSharedCheck_164_ == 0)
{
v___x_157_ = v_x_131_;
v_isShared_158_ = v_isSharedCheck_164_;
goto v_resetjp_156_;
}
else
{
lean_inc(v_a_155_);
lean_inc(v_a_154_);
lean_dec(v_x_131_);
v___x_157_ = lean_box(0);
v_isShared_158_ = v_isSharedCheck_164_;
goto v_resetjp_156_;
}
v_resetjp_156_:
{
lean_object* v___x_159_; lean_object* v___x_160_; lean_object* v___x_162_; 
v___x_159_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_shift(v_d_129_, v_c_130_, v_a_154_);
v___x_160_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_shift(v_d_129_, v_c_130_, v_a_155_);
if (v_isShared_158_ == 0)
{
lean_ctor_set(v___x_157_, 1, v___x_160_);
lean_ctor_set(v___x_157_, 0, v___x_159_);
v___x_162_ = v___x_157_;
goto v_reusejp_161_;
}
else
{
lean_object* v_reuseFailAlloc_163_; 
v_reuseFailAlloc_163_ = lean_alloc_ctor(2, 2, 0);
lean_ctor_set(v_reuseFailAlloc_163_, 0, v___x_159_);
lean_ctor_set(v_reuseFailAlloc_163_, 1, v___x_160_);
v___x_162_ = v_reuseFailAlloc_163_;
goto v_reusejp_161_;
}
v_reusejp_161_:
{
return v___x_162_;
}
}
}
case 3:
{
lean_object* v_a_165_; lean_object* v_a_166_; lean_object* v___x_168_; uint8_t v_isShared_169_; uint8_t v_isSharedCheck_175_; 
v_a_165_ = lean_ctor_get(v_x_131_, 0);
v_a_166_ = lean_ctor_get(v_x_131_, 1);
v_isSharedCheck_175_ = !lean_is_exclusive(v_x_131_);
if (v_isSharedCheck_175_ == 0)
{
v___x_168_ = v_x_131_;
v_isShared_169_ = v_isSharedCheck_175_;
goto v_resetjp_167_;
}
else
{
lean_inc(v_a_166_);
lean_inc(v_a_165_);
lean_dec(v_x_131_);
v___x_168_ = lean_box(0);
v_isShared_169_ = v_isSharedCheck_175_;
goto v_resetjp_167_;
}
v_resetjp_167_:
{
lean_object* v___x_170_; lean_object* v___x_171_; lean_object* v___x_173_; 
v___x_170_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_shift(v_d_129_, v_c_130_, v_a_165_);
v___x_171_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_shift(v_d_129_, v_c_130_, v_a_166_);
if (v_isShared_169_ == 0)
{
lean_ctor_set(v___x_168_, 1, v___x_171_);
lean_ctor_set(v___x_168_, 0, v___x_170_);
v___x_173_ = v___x_168_;
goto v_reusejp_172_;
}
else
{
lean_object* v_reuseFailAlloc_174_; 
v_reuseFailAlloc_174_ = lean_alloc_ctor(3, 2, 0);
lean_ctor_set(v_reuseFailAlloc_174_, 0, v___x_170_);
lean_ctor_set(v_reuseFailAlloc_174_, 1, v___x_171_);
v___x_173_ = v_reuseFailAlloc_174_;
goto v_reusejp_172_;
}
v_reusejp_172_:
{
return v___x_173_;
}
}
}
case 4:
{
lean_object* v_a_176_; lean_object* v___x_178_; uint8_t v_isShared_179_; uint8_t v_isSharedCheck_184_; 
v_a_176_ = lean_ctor_get(v_x_131_, 0);
v_isSharedCheck_184_ = !lean_is_exclusive(v_x_131_);
if (v_isSharedCheck_184_ == 0)
{
v___x_178_ = v_x_131_;
v_isShared_179_ = v_isSharedCheck_184_;
goto v_resetjp_177_;
}
else
{
lean_inc(v_a_176_);
lean_dec(v_x_131_);
v___x_178_ = lean_box(0);
v_isShared_179_ = v_isSharedCheck_184_;
goto v_resetjp_177_;
}
v_resetjp_177_:
{
lean_object* v___x_180_; lean_object* v___x_182_; 
v___x_180_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_shift(v_d_129_, v_c_130_, v_a_176_);
if (v_isShared_179_ == 0)
{
lean_ctor_set(v___x_178_, 0, v___x_180_);
v___x_182_ = v___x_178_;
goto v_reusejp_181_;
}
else
{
lean_object* v_reuseFailAlloc_183_; 
v_reuseFailAlloc_183_ = lean_alloc_ctor(4, 1, 0);
lean_ctor_set(v_reuseFailAlloc_183_, 0, v___x_180_);
v___x_182_ = v_reuseFailAlloc_183_;
goto v_reusejp_181_;
}
v_reusejp_181_:
{
return v___x_182_;
}
}
}
case 5:
{
lean_object* v_a_185_; lean_object* v___x_187_; uint8_t v_isShared_188_; uint8_t v_isSharedCheck_193_; 
v_a_185_ = lean_ctor_get(v_x_131_, 0);
v_isSharedCheck_193_ = !lean_is_exclusive(v_x_131_);
if (v_isSharedCheck_193_ == 0)
{
v___x_187_ = v_x_131_;
v_isShared_188_ = v_isSharedCheck_193_;
goto v_resetjp_186_;
}
else
{
lean_inc(v_a_185_);
lean_dec(v_x_131_);
v___x_187_ = lean_box(0);
v_isShared_188_ = v_isSharedCheck_193_;
goto v_resetjp_186_;
}
v_resetjp_186_:
{
lean_object* v___x_189_; lean_object* v___x_191_; 
v___x_189_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_shift(v_d_129_, v_c_130_, v_a_185_);
if (v_isShared_188_ == 0)
{
lean_ctor_set(v___x_187_, 0, v___x_189_);
v___x_191_ = v___x_187_;
goto v_reusejp_190_;
}
else
{
lean_object* v_reuseFailAlloc_192_; 
v_reuseFailAlloc_192_ = lean_alloc_ctor(5, 1, 0);
lean_ctor_set(v_reuseFailAlloc_192_, 0, v___x_189_);
v___x_191_ = v_reuseFailAlloc_192_;
goto v_reusejp_190_;
}
v_reusejp_190_:
{
return v___x_191_;
}
}
}
case 6:
{
lean_object* v_a_194_; lean_object* v___x_196_; uint8_t v_isShared_197_; uint8_t v_isSharedCheck_202_; 
v_a_194_ = lean_ctor_get(v_x_131_, 0);
v_isSharedCheck_202_ = !lean_is_exclusive(v_x_131_);
if (v_isSharedCheck_202_ == 0)
{
v___x_196_ = v_x_131_;
v_isShared_197_ = v_isSharedCheck_202_;
goto v_resetjp_195_;
}
else
{
lean_inc(v_a_194_);
lean_dec(v_x_131_);
v___x_196_ = lean_box(0);
v_isShared_197_ = v_isSharedCheck_202_;
goto v_resetjp_195_;
}
v_resetjp_195_:
{
lean_object* v___x_198_; lean_object* v___x_200_; 
v___x_198_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_shift(v_d_129_, v_c_130_, v_a_194_);
if (v_isShared_197_ == 0)
{
lean_ctor_set(v___x_196_, 0, v___x_198_);
v___x_200_ = v___x_196_;
goto v_reusejp_199_;
}
else
{
lean_object* v_reuseFailAlloc_201_; 
v_reuseFailAlloc_201_ = lean_alloc_ctor(6, 1, 0);
lean_ctor_set(v_reuseFailAlloc_201_, 0, v___x_198_);
v___x_200_ = v_reuseFailAlloc_201_;
goto v_reusejp_199_;
}
v_reusejp_199_:
{
return v___x_200_;
}
}
}
case 7:
{
lean_object* v_a_203_; lean_object* v___x_205_; uint8_t v_isShared_206_; uint8_t v_isSharedCheck_211_; 
v_a_203_ = lean_ctor_get(v_x_131_, 0);
v_isSharedCheck_211_ = !lean_is_exclusive(v_x_131_);
if (v_isSharedCheck_211_ == 0)
{
v___x_205_ = v_x_131_;
v_isShared_206_ = v_isSharedCheck_211_;
goto v_resetjp_204_;
}
else
{
lean_inc(v_a_203_);
lean_dec(v_x_131_);
v___x_205_ = lean_box(0);
v_isShared_206_ = v_isSharedCheck_211_;
goto v_resetjp_204_;
}
v_resetjp_204_:
{
lean_object* v___x_207_; lean_object* v___x_209_; 
v___x_207_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_shift(v_d_129_, v_c_130_, v_a_203_);
if (v_isShared_206_ == 0)
{
lean_ctor_set(v___x_205_, 0, v___x_207_);
v___x_209_ = v___x_205_;
goto v_reusejp_208_;
}
else
{
lean_object* v_reuseFailAlloc_210_; 
v_reuseFailAlloc_210_ = lean_alloc_ctor(7, 1, 0);
lean_ctor_set(v_reuseFailAlloc_210_, 0, v___x_207_);
v___x_209_ = v_reuseFailAlloc_210_;
goto v_reusejp_208_;
}
v_reusejp_208_:
{
return v___x_209_;
}
}
}
case 8:
{
lean_object* v_a_212_; lean_object* v_a_213_; lean_object* v_a_214_; lean_object* v___x_216_; uint8_t v_isShared_217_; uint8_t v_isSharedCheck_226_; 
v_a_212_ = lean_ctor_get(v_x_131_, 0);
v_a_213_ = lean_ctor_get(v_x_131_, 1);
v_a_214_ = lean_ctor_get(v_x_131_, 2);
v_isSharedCheck_226_ = !lean_is_exclusive(v_x_131_);
if (v_isSharedCheck_226_ == 0)
{
v___x_216_ = v_x_131_;
v_isShared_217_ = v_isSharedCheck_226_;
goto v_resetjp_215_;
}
else
{
lean_inc(v_a_214_);
lean_inc(v_a_213_);
lean_inc(v_a_212_);
lean_dec(v_x_131_);
v___x_216_ = lean_box(0);
v_isShared_217_ = v_isSharedCheck_226_;
goto v_resetjp_215_;
}
v_resetjp_215_:
{
lean_object* v___x_218_; lean_object* v___x_219_; lean_object* v___x_220_; lean_object* v___x_221_; lean_object* v___x_222_; lean_object* v___x_224_; 
v___x_218_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_shift(v_d_129_, v_c_130_, v_a_212_);
v___x_219_ = lean_unsigned_to_nat(1u);
v___x_220_ = lean_nat_add(v_c_130_, v___x_219_);
v___x_221_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_shift(v_d_129_, v___x_220_, v_a_213_);
v___x_222_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_shift(v_d_129_, v___x_220_, v_a_214_);
lean_dec(v___x_220_);
if (v_isShared_217_ == 0)
{
lean_ctor_set(v___x_216_, 2, v___x_222_);
lean_ctor_set(v___x_216_, 1, v___x_221_);
lean_ctor_set(v___x_216_, 0, v___x_218_);
v___x_224_ = v___x_216_;
goto v_reusejp_223_;
}
else
{
lean_object* v_reuseFailAlloc_225_; 
v_reuseFailAlloc_225_ = lean_alloc_ctor(8, 3, 0);
lean_ctor_set(v_reuseFailAlloc_225_, 0, v___x_218_);
lean_ctor_set(v_reuseFailAlloc_225_, 1, v___x_221_);
lean_ctor_set(v_reuseFailAlloc_225_, 2, v___x_222_);
v___x_224_ = v_reuseFailAlloc_225_;
goto v_reusejp_223_;
}
v_reusejp_223_:
{
return v___x_224_;
}
}
}
case 9:
{
lean_object* v_a_227_; lean_object* v___x_229_; uint8_t v_isShared_230_; uint8_t v_isSharedCheck_237_; 
v_a_227_ = lean_ctor_get(v_x_131_, 0);
v_isSharedCheck_237_ = !lean_is_exclusive(v_x_131_);
if (v_isSharedCheck_237_ == 0)
{
v___x_229_ = v_x_131_;
v_isShared_230_ = v_isSharedCheck_237_;
goto v_resetjp_228_;
}
else
{
lean_inc(v_a_227_);
lean_dec(v_x_131_);
v___x_229_ = lean_box(0);
v_isShared_230_ = v_isSharedCheck_237_;
goto v_resetjp_228_;
}
v_resetjp_228_:
{
lean_object* v___x_231_; lean_object* v___x_232_; lean_object* v___x_233_; lean_object* v___x_235_; 
v___x_231_ = lean_unsigned_to_nat(1u);
v___x_232_ = lean_nat_add(v_c_130_, v___x_231_);
v___x_233_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_shift(v_d_129_, v___x_232_, v_a_227_);
lean_dec(v___x_232_);
if (v_isShared_230_ == 0)
{
lean_ctor_set(v___x_229_, 0, v___x_233_);
v___x_235_ = v___x_229_;
goto v_reusejp_234_;
}
else
{
lean_object* v_reuseFailAlloc_236_; 
v_reuseFailAlloc_236_ = lean_alloc_ctor(9, 1, 0);
lean_ctor_set(v_reuseFailAlloc_236_, 0, v___x_233_);
v___x_235_ = v_reuseFailAlloc_236_;
goto v_reusejp_234_;
}
v_reusejp_234_:
{
return v___x_235_;
}
}
}
default: 
{
return v_x_131_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_shift___boxed(lean_object* v_d_238_, lean_object* v_c_239_, lean_object* v_x_240_){
_start:
{
lean_object* v_res_241_; 
v_res_241_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_shift(v_d_238_, v_c_239_, v_x_240_);
lean_dec(v_c_239_);
lean_dec(v_d_238_);
return v_res_241_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_substAux(lean_object* v_e_242_, lean_object* v_depth_243_, lean_object* v_x_244_){
_start:
{
switch(lean_obj_tag(v_x_244_))
{
case 0:
{
lean_object* v_a_245_; uint8_t v___x_246_; 
v_a_245_ = lean_ctor_get(v_x_244_, 0);
v___x_246_ = lean_nat_dec_lt(v_a_245_, v_depth_243_);
if (v___x_246_ == 0)
{
lean_object* v___x_248_; uint8_t v_isShared_249_; uint8_t v_isSharedCheck_258_; 
lean_inc(v_a_245_);
v_isSharedCheck_258_ = !lean_is_exclusive(v_x_244_);
if (v_isSharedCheck_258_ == 0)
{
lean_object* v_unused_259_; 
v_unused_259_ = lean_ctor_get(v_x_244_, 0);
lean_dec(v_unused_259_);
v___x_248_ = v_x_244_;
v_isShared_249_ = v_isSharedCheck_258_;
goto v_resetjp_247_;
}
else
{
lean_dec(v_x_244_);
v___x_248_ = lean_box(0);
v_isShared_249_ = v_isSharedCheck_258_;
goto v_resetjp_247_;
}
v_resetjp_247_:
{
uint8_t v___x_250_; 
v___x_250_ = lean_nat_dec_eq(v_a_245_, v_depth_243_);
if (v___x_250_ == 0)
{
lean_object* v___x_251_; lean_object* v___x_252_; lean_object* v___x_254_; 
lean_dec(v_e_242_);
v___x_251_ = lean_unsigned_to_nat(1u);
v___x_252_ = lean_nat_sub(v_a_245_, v___x_251_);
lean_dec(v_a_245_);
if (v_isShared_249_ == 0)
{
lean_ctor_set(v___x_248_, 0, v___x_252_);
v___x_254_ = v___x_248_;
goto v_reusejp_253_;
}
else
{
lean_object* v_reuseFailAlloc_255_; 
v_reuseFailAlloc_255_ = lean_alloc_ctor(0, 1, 0);
lean_ctor_set(v_reuseFailAlloc_255_, 0, v___x_252_);
v___x_254_ = v_reuseFailAlloc_255_;
goto v_reusejp_253_;
}
v_reusejp_253_:
{
return v___x_254_;
}
}
else
{
lean_object* v___x_256_; lean_object* v___x_257_; 
lean_del_object(v___x_248_);
lean_dec(v_a_245_);
v___x_256_ = lean_unsigned_to_nat(0u);
v___x_257_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_shift(v_depth_243_, v___x_256_, v_e_242_);
return v___x_257_;
}
}
}
else
{
lean_dec(v_e_242_);
return v_x_244_;
}
}
case 1:
{
lean_object* v_a_260_; lean_object* v___x_262_; uint8_t v_isShared_263_; uint8_t v_isSharedCheck_270_; 
v_a_260_ = lean_ctor_get(v_x_244_, 0);
v_isSharedCheck_270_ = !lean_is_exclusive(v_x_244_);
if (v_isSharedCheck_270_ == 0)
{
v___x_262_ = v_x_244_;
v_isShared_263_ = v_isSharedCheck_270_;
goto v_resetjp_261_;
}
else
{
lean_inc(v_a_260_);
lean_dec(v_x_244_);
v___x_262_ = lean_box(0);
v_isShared_263_ = v_isSharedCheck_270_;
goto v_resetjp_261_;
}
v_resetjp_261_:
{
lean_object* v___x_264_; lean_object* v___x_265_; lean_object* v___x_266_; lean_object* v___x_268_; 
v___x_264_ = lean_unsigned_to_nat(1u);
v___x_265_ = lean_nat_add(v_depth_243_, v___x_264_);
v___x_266_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_substAux(v_e_242_, v___x_265_, v_a_260_);
lean_dec(v___x_265_);
if (v_isShared_263_ == 0)
{
lean_ctor_set(v___x_262_, 0, v___x_266_);
v___x_268_ = v___x_262_;
goto v_reusejp_267_;
}
else
{
lean_object* v_reuseFailAlloc_269_; 
v_reuseFailAlloc_269_ = lean_alloc_ctor(1, 1, 0);
lean_ctor_set(v_reuseFailAlloc_269_, 0, v___x_266_);
v___x_268_ = v_reuseFailAlloc_269_;
goto v_reusejp_267_;
}
v_reusejp_267_:
{
return v___x_268_;
}
}
}
case 2:
{
lean_object* v_a_271_; lean_object* v_a_272_; lean_object* v___x_274_; uint8_t v_isShared_275_; uint8_t v_isSharedCheck_281_; 
v_a_271_ = lean_ctor_get(v_x_244_, 0);
v_a_272_ = lean_ctor_get(v_x_244_, 1);
v_isSharedCheck_281_ = !lean_is_exclusive(v_x_244_);
if (v_isSharedCheck_281_ == 0)
{
v___x_274_ = v_x_244_;
v_isShared_275_ = v_isSharedCheck_281_;
goto v_resetjp_273_;
}
else
{
lean_inc(v_a_272_);
lean_inc(v_a_271_);
lean_dec(v_x_244_);
v___x_274_ = lean_box(0);
v_isShared_275_ = v_isSharedCheck_281_;
goto v_resetjp_273_;
}
v_resetjp_273_:
{
lean_object* v___x_276_; lean_object* v___x_277_; lean_object* v___x_279_; 
lean_inc(v_e_242_);
v___x_276_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_substAux(v_e_242_, v_depth_243_, v_a_271_);
v___x_277_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_substAux(v_e_242_, v_depth_243_, v_a_272_);
if (v_isShared_275_ == 0)
{
lean_ctor_set(v___x_274_, 1, v___x_277_);
lean_ctor_set(v___x_274_, 0, v___x_276_);
v___x_279_ = v___x_274_;
goto v_reusejp_278_;
}
else
{
lean_object* v_reuseFailAlloc_280_; 
v_reuseFailAlloc_280_ = lean_alloc_ctor(2, 2, 0);
lean_ctor_set(v_reuseFailAlloc_280_, 0, v___x_276_);
lean_ctor_set(v_reuseFailAlloc_280_, 1, v___x_277_);
v___x_279_ = v_reuseFailAlloc_280_;
goto v_reusejp_278_;
}
v_reusejp_278_:
{
return v___x_279_;
}
}
}
case 3:
{
lean_object* v_a_282_; lean_object* v_a_283_; lean_object* v___x_285_; uint8_t v_isShared_286_; uint8_t v_isSharedCheck_292_; 
v_a_282_ = lean_ctor_get(v_x_244_, 0);
v_a_283_ = lean_ctor_get(v_x_244_, 1);
v_isSharedCheck_292_ = !lean_is_exclusive(v_x_244_);
if (v_isSharedCheck_292_ == 0)
{
v___x_285_ = v_x_244_;
v_isShared_286_ = v_isSharedCheck_292_;
goto v_resetjp_284_;
}
else
{
lean_inc(v_a_283_);
lean_inc(v_a_282_);
lean_dec(v_x_244_);
v___x_285_ = lean_box(0);
v_isShared_286_ = v_isSharedCheck_292_;
goto v_resetjp_284_;
}
v_resetjp_284_:
{
lean_object* v___x_287_; lean_object* v___x_288_; lean_object* v___x_290_; 
lean_inc(v_e_242_);
v___x_287_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_substAux(v_e_242_, v_depth_243_, v_a_282_);
v___x_288_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_substAux(v_e_242_, v_depth_243_, v_a_283_);
if (v_isShared_286_ == 0)
{
lean_ctor_set(v___x_285_, 1, v___x_288_);
lean_ctor_set(v___x_285_, 0, v___x_287_);
v___x_290_ = v___x_285_;
goto v_reusejp_289_;
}
else
{
lean_object* v_reuseFailAlloc_291_; 
v_reuseFailAlloc_291_ = lean_alloc_ctor(3, 2, 0);
lean_ctor_set(v_reuseFailAlloc_291_, 0, v___x_287_);
lean_ctor_set(v_reuseFailAlloc_291_, 1, v___x_288_);
v___x_290_ = v_reuseFailAlloc_291_;
goto v_reusejp_289_;
}
v_reusejp_289_:
{
return v___x_290_;
}
}
}
case 4:
{
lean_object* v_a_293_; lean_object* v___x_295_; uint8_t v_isShared_296_; uint8_t v_isSharedCheck_301_; 
v_a_293_ = lean_ctor_get(v_x_244_, 0);
v_isSharedCheck_301_ = !lean_is_exclusive(v_x_244_);
if (v_isSharedCheck_301_ == 0)
{
v___x_295_ = v_x_244_;
v_isShared_296_ = v_isSharedCheck_301_;
goto v_resetjp_294_;
}
else
{
lean_inc(v_a_293_);
lean_dec(v_x_244_);
v___x_295_ = lean_box(0);
v_isShared_296_ = v_isSharedCheck_301_;
goto v_resetjp_294_;
}
v_resetjp_294_:
{
lean_object* v___x_297_; lean_object* v___x_299_; 
v___x_297_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_substAux(v_e_242_, v_depth_243_, v_a_293_);
if (v_isShared_296_ == 0)
{
lean_ctor_set(v___x_295_, 0, v___x_297_);
v___x_299_ = v___x_295_;
goto v_reusejp_298_;
}
else
{
lean_object* v_reuseFailAlloc_300_; 
v_reuseFailAlloc_300_ = lean_alloc_ctor(4, 1, 0);
lean_ctor_set(v_reuseFailAlloc_300_, 0, v___x_297_);
v___x_299_ = v_reuseFailAlloc_300_;
goto v_reusejp_298_;
}
v_reusejp_298_:
{
return v___x_299_;
}
}
}
case 5:
{
lean_object* v_a_302_; lean_object* v___x_304_; uint8_t v_isShared_305_; uint8_t v_isSharedCheck_310_; 
v_a_302_ = lean_ctor_get(v_x_244_, 0);
v_isSharedCheck_310_ = !lean_is_exclusive(v_x_244_);
if (v_isSharedCheck_310_ == 0)
{
v___x_304_ = v_x_244_;
v_isShared_305_ = v_isSharedCheck_310_;
goto v_resetjp_303_;
}
else
{
lean_inc(v_a_302_);
lean_dec(v_x_244_);
v___x_304_ = lean_box(0);
v_isShared_305_ = v_isSharedCheck_310_;
goto v_resetjp_303_;
}
v_resetjp_303_:
{
lean_object* v___x_306_; lean_object* v___x_308_; 
v___x_306_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_substAux(v_e_242_, v_depth_243_, v_a_302_);
if (v_isShared_305_ == 0)
{
lean_ctor_set(v___x_304_, 0, v___x_306_);
v___x_308_ = v___x_304_;
goto v_reusejp_307_;
}
else
{
lean_object* v_reuseFailAlloc_309_; 
v_reuseFailAlloc_309_ = lean_alloc_ctor(5, 1, 0);
lean_ctor_set(v_reuseFailAlloc_309_, 0, v___x_306_);
v___x_308_ = v_reuseFailAlloc_309_;
goto v_reusejp_307_;
}
v_reusejp_307_:
{
return v___x_308_;
}
}
}
case 6:
{
lean_object* v_a_311_; lean_object* v___x_313_; uint8_t v_isShared_314_; uint8_t v_isSharedCheck_319_; 
v_a_311_ = lean_ctor_get(v_x_244_, 0);
v_isSharedCheck_319_ = !lean_is_exclusive(v_x_244_);
if (v_isSharedCheck_319_ == 0)
{
v___x_313_ = v_x_244_;
v_isShared_314_ = v_isSharedCheck_319_;
goto v_resetjp_312_;
}
else
{
lean_inc(v_a_311_);
lean_dec(v_x_244_);
v___x_313_ = lean_box(0);
v_isShared_314_ = v_isSharedCheck_319_;
goto v_resetjp_312_;
}
v_resetjp_312_:
{
lean_object* v___x_315_; lean_object* v___x_317_; 
v___x_315_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_substAux(v_e_242_, v_depth_243_, v_a_311_);
if (v_isShared_314_ == 0)
{
lean_ctor_set(v___x_313_, 0, v___x_315_);
v___x_317_ = v___x_313_;
goto v_reusejp_316_;
}
else
{
lean_object* v_reuseFailAlloc_318_; 
v_reuseFailAlloc_318_ = lean_alloc_ctor(6, 1, 0);
lean_ctor_set(v_reuseFailAlloc_318_, 0, v___x_315_);
v___x_317_ = v_reuseFailAlloc_318_;
goto v_reusejp_316_;
}
v_reusejp_316_:
{
return v___x_317_;
}
}
}
case 7:
{
lean_object* v_a_320_; lean_object* v___x_322_; uint8_t v_isShared_323_; uint8_t v_isSharedCheck_328_; 
v_a_320_ = lean_ctor_get(v_x_244_, 0);
v_isSharedCheck_328_ = !lean_is_exclusive(v_x_244_);
if (v_isSharedCheck_328_ == 0)
{
v___x_322_ = v_x_244_;
v_isShared_323_ = v_isSharedCheck_328_;
goto v_resetjp_321_;
}
else
{
lean_inc(v_a_320_);
lean_dec(v_x_244_);
v___x_322_ = lean_box(0);
v_isShared_323_ = v_isSharedCheck_328_;
goto v_resetjp_321_;
}
v_resetjp_321_:
{
lean_object* v___x_324_; lean_object* v___x_326_; 
v___x_324_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_substAux(v_e_242_, v_depth_243_, v_a_320_);
if (v_isShared_323_ == 0)
{
lean_ctor_set(v___x_322_, 0, v___x_324_);
v___x_326_ = v___x_322_;
goto v_reusejp_325_;
}
else
{
lean_object* v_reuseFailAlloc_327_; 
v_reuseFailAlloc_327_ = lean_alloc_ctor(7, 1, 0);
lean_ctor_set(v_reuseFailAlloc_327_, 0, v___x_324_);
v___x_326_ = v_reuseFailAlloc_327_;
goto v_reusejp_325_;
}
v_reusejp_325_:
{
return v___x_326_;
}
}
}
case 8:
{
lean_object* v_a_329_; lean_object* v_a_330_; lean_object* v_a_331_; lean_object* v___x_333_; uint8_t v_isShared_334_; uint8_t v_isSharedCheck_343_; 
v_a_329_ = lean_ctor_get(v_x_244_, 0);
v_a_330_ = lean_ctor_get(v_x_244_, 1);
v_a_331_ = lean_ctor_get(v_x_244_, 2);
v_isSharedCheck_343_ = !lean_is_exclusive(v_x_244_);
if (v_isSharedCheck_343_ == 0)
{
v___x_333_ = v_x_244_;
v_isShared_334_ = v_isSharedCheck_343_;
goto v_resetjp_332_;
}
else
{
lean_inc(v_a_331_);
lean_inc(v_a_330_);
lean_inc(v_a_329_);
lean_dec(v_x_244_);
v___x_333_ = lean_box(0);
v_isShared_334_ = v_isSharedCheck_343_;
goto v_resetjp_332_;
}
v_resetjp_332_:
{
lean_object* v___x_335_; lean_object* v___x_336_; lean_object* v___x_337_; lean_object* v___x_338_; lean_object* v___x_339_; lean_object* v___x_341_; 
lean_inc_n(v_e_242_, 2);
v___x_335_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_substAux(v_e_242_, v_depth_243_, v_a_329_);
v___x_336_ = lean_unsigned_to_nat(1u);
v___x_337_ = lean_nat_add(v_depth_243_, v___x_336_);
v___x_338_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_substAux(v_e_242_, v___x_337_, v_a_330_);
v___x_339_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_substAux(v_e_242_, v___x_337_, v_a_331_);
lean_dec(v___x_337_);
if (v_isShared_334_ == 0)
{
lean_ctor_set(v___x_333_, 2, v___x_339_);
lean_ctor_set(v___x_333_, 1, v___x_338_);
lean_ctor_set(v___x_333_, 0, v___x_335_);
v___x_341_ = v___x_333_;
goto v_reusejp_340_;
}
else
{
lean_object* v_reuseFailAlloc_342_; 
v_reuseFailAlloc_342_ = lean_alloc_ctor(8, 3, 0);
lean_ctor_set(v_reuseFailAlloc_342_, 0, v___x_335_);
lean_ctor_set(v_reuseFailAlloc_342_, 1, v___x_338_);
lean_ctor_set(v_reuseFailAlloc_342_, 2, v___x_339_);
v___x_341_ = v_reuseFailAlloc_342_;
goto v_reusejp_340_;
}
v_reusejp_340_:
{
return v___x_341_;
}
}
}
case 9:
{
lean_object* v_a_344_; lean_object* v___x_346_; uint8_t v_isShared_347_; uint8_t v_isSharedCheck_354_; 
v_a_344_ = lean_ctor_get(v_x_244_, 0);
v_isSharedCheck_354_ = !lean_is_exclusive(v_x_244_);
if (v_isSharedCheck_354_ == 0)
{
v___x_346_ = v_x_244_;
v_isShared_347_ = v_isSharedCheck_354_;
goto v_resetjp_345_;
}
else
{
lean_inc(v_a_344_);
lean_dec(v_x_244_);
v___x_346_ = lean_box(0);
v_isShared_347_ = v_isSharedCheck_354_;
goto v_resetjp_345_;
}
v_resetjp_345_:
{
lean_object* v___x_348_; lean_object* v___x_349_; lean_object* v___x_350_; lean_object* v___x_352_; 
v___x_348_ = lean_unsigned_to_nat(1u);
v___x_349_ = lean_nat_add(v_depth_243_, v___x_348_);
v___x_350_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_substAux(v_e_242_, v___x_349_, v_a_344_);
lean_dec(v___x_349_);
if (v_isShared_347_ == 0)
{
lean_ctor_set(v___x_346_, 0, v___x_350_);
v___x_352_ = v___x_346_;
goto v_reusejp_351_;
}
else
{
lean_object* v_reuseFailAlloc_353_; 
v_reuseFailAlloc_353_ = lean_alloc_ctor(9, 1, 0);
lean_ctor_set(v_reuseFailAlloc_353_, 0, v___x_350_);
v___x_352_ = v_reuseFailAlloc_353_;
goto v_reusejp_351_;
}
v_reusejp_351_:
{
return v___x_352_;
}
}
}
default: 
{
lean_dec(v_e_242_);
return v_x_244_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_substAux___boxed(lean_object* v_e_355_, lean_object* v_depth_356_, lean_object* v_x_357_){
_start:
{
lean_object* v_res_358_; 
v_res_358_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_substAux(v_e_355_, v_depth_356_, v_x_357_);
lean_dec(v_depth_356_);
return v_res_358_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_subst0(lean_object* v_e_359_, lean_object* v_body_360_){
_start:
{
lean_object* v___x_361_; lean_object* v___x_362_; 
v___x_361_ = lean_unsigned_to_nat(0u);
v___x_362_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_substAux(v_e_359_, v___x_361_, v_body_360_);
return v___x_362_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_lamSize(lean_object* v_x_363_){
_start:
{
lean_object* v_b_365_; lean_object* v_f_370_; lean_object* v_a_371_; 
switch(lean_obj_tag(v_x_363_))
{
case 1:
{
lean_object* v_a_377_; 
v_a_377_ = lean_ctor_get(v_x_363_, 0);
v_b_365_ = v_a_377_;
goto v___jp_364_;
}
case 2:
{
lean_object* v_a_378_; lean_object* v_a_379_; 
v_a_378_ = lean_ctor_get(v_x_363_, 0);
v_a_379_ = lean_ctor_get(v_x_363_, 1);
v_f_370_ = v_a_378_;
v_a_371_ = v_a_379_;
goto v___jp_369_;
}
case 3:
{
lean_object* v_a_380_; lean_object* v_a_381_; 
v_a_380_ = lean_ctor_get(v_x_363_, 0);
v_a_381_ = lean_ctor_get(v_x_363_, 1);
v_f_370_ = v_a_380_;
v_a_371_ = v_a_381_;
goto v___jp_369_;
}
case 4:
{
lean_object* v_a_382_; 
v_a_382_ = lean_ctor_get(v_x_363_, 0);
v_b_365_ = v_a_382_;
goto v___jp_364_;
}
case 5:
{
lean_object* v_a_383_; 
v_a_383_ = lean_ctor_get(v_x_363_, 0);
v_b_365_ = v_a_383_;
goto v___jp_364_;
}
case 6:
{
lean_object* v_a_384_; 
v_a_384_ = lean_ctor_get(v_x_363_, 0);
v_b_365_ = v_a_384_;
goto v___jp_364_;
}
case 7:
{
lean_object* v_a_385_; 
v_a_385_ = lean_ctor_get(v_x_363_, 0);
v_b_365_ = v_a_385_;
goto v___jp_364_;
}
case 8:
{
lean_object* v_a_386_; lean_object* v_a_387_; lean_object* v_a_388_; lean_object* v___x_389_; lean_object* v___x_390_; lean_object* v___x_391_; lean_object* v___x_392_; lean_object* v___x_393_; lean_object* v___x_394_; lean_object* v___x_395_; 
v_a_386_ = lean_ctor_get(v_x_363_, 0);
v_a_387_ = lean_ctor_get(v_x_363_, 1);
v_a_388_ = lean_ctor_get(v_x_363_, 2);
v___x_389_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_lamSize(v_a_386_);
v___x_390_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_lamSize(v_a_387_);
v___x_391_ = lean_nat_add(v___x_389_, v___x_390_);
lean_dec(v___x_390_);
lean_dec(v___x_389_);
v___x_392_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_lamSize(v_a_388_);
v___x_393_ = lean_nat_add(v___x_391_, v___x_392_);
lean_dec(v___x_392_);
lean_dec(v___x_391_);
v___x_394_ = lean_unsigned_to_nat(1u);
v___x_395_ = lean_nat_add(v___x_393_, v___x_394_);
lean_dec(v___x_393_);
return v___x_395_;
}
case 9:
{
lean_object* v_a_396_; 
v_a_396_ = lean_ctor_get(v_x_363_, 0);
v_b_365_ = v_a_396_;
goto v___jp_364_;
}
default: 
{
lean_object* v___x_397_; 
v___x_397_ = lean_unsigned_to_nat(1u);
return v___x_397_;
}
}
v___jp_364_:
{
lean_object* v___x_366_; lean_object* v___x_367_; lean_object* v___x_368_; 
v___x_366_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_lamSize(v_b_365_);
v___x_367_ = lean_unsigned_to_nat(1u);
v___x_368_ = lean_nat_add(v___x_366_, v___x_367_);
lean_dec(v___x_366_);
return v___x_368_;
}
v___jp_369_:
{
lean_object* v___x_372_; lean_object* v___x_373_; lean_object* v___x_374_; lean_object* v___x_375_; lean_object* v___x_376_; 
v___x_372_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_lamSize(v_f_370_);
v___x_373_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_lamSize(v_a_371_);
v___x_374_ = lean_nat_add(v___x_372_, v___x_373_);
lean_dec(v___x_373_);
lean_dec(v___x_372_);
v___x_375_ = lean_unsigned_to_nat(1u);
v___x_376_ = lean_nat_add(v___x_374_, v___x_375_);
lean_dec(v___x_374_);
return v___x_376_;
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_lamSize___boxed(lean_object* v_x_398_){
_start:
{
lean_object* v_res_399_; 
v_res_399_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_lamSize(v_x_398_);
lean_dec(v_x_398_);
return v_res_399_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_Translation_0__TranslationAsymmetry_LamTerm_shift_match__1_splitter___redArg(lean_object* v_x_400_, lean_object* v_h__1_401_, lean_object* v_h__2_402_, lean_object* v_h__3_403_, lean_object* v_h__4_404_, lean_object* v_h__5_405_, lean_object* v_h__6_406_, lean_object* v_h__7_407_, lean_object* v_h__8_408_, lean_object* v_h__9_409_, lean_object* v_h__10_410_, lean_object* v_h__11_411_){
_start:
{
switch(lean_obj_tag(v_x_400_))
{
case 0:
{
lean_object* v_a_412_; lean_object* v___x_413_; 
lean_dec(v_h__11_411_);
lean_dec(v_h__10_410_);
lean_dec(v_h__9_409_);
lean_dec(v_h__8_408_);
lean_dec(v_h__7_407_);
lean_dec(v_h__6_406_);
lean_dec(v_h__5_405_);
lean_dec(v_h__4_404_);
lean_dec(v_h__3_403_);
lean_dec(v_h__2_402_);
v_a_412_ = lean_ctor_get(v_x_400_, 0);
lean_inc(v_a_412_);
lean_dec_ref_known(v_x_400_, 1);
v___x_413_ = lean_apply_1(v_h__1_401_, v_a_412_);
return v___x_413_;
}
case 1:
{
lean_object* v_a_414_; lean_object* v___x_415_; 
lean_dec(v_h__11_411_);
lean_dec(v_h__10_410_);
lean_dec(v_h__9_409_);
lean_dec(v_h__8_408_);
lean_dec(v_h__7_407_);
lean_dec(v_h__6_406_);
lean_dec(v_h__5_405_);
lean_dec(v_h__4_404_);
lean_dec(v_h__3_403_);
lean_dec(v_h__1_401_);
v_a_414_ = lean_ctor_get(v_x_400_, 0);
lean_inc(v_a_414_);
lean_dec_ref_known(v_x_400_, 1);
v___x_415_ = lean_apply_1(v_h__2_402_, v_a_414_);
return v___x_415_;
}
case 2:
{
lean_object* v_a_416_; lean_object* v_a_417_; lean_object* v___x_418_; 
lean_dec(v_h__11_411_);
lean_dec(v_h__10_410_);
lean_dec(v_h__9_409_);
lean_dec(v_h__8_408_);
lean_dec(v_h__7_407_);
lean_dec(v_h__6_406_);
lean_dec(v_h__5_405_);
lean_dec(v_h__4_404_);
lean_dec(v_h__2_402_);
lean_dec(v_h__1_401_);
v_a_416_ = lean_ctor_get(v_x_400_, 0);
lean_inc(v_a_416_);
v_a_417_ = lean_ctor_get(v_x_400_, 1);
lean_inc(v_a_417_);
lean_dec_ref_known(v_x_400_, 2);
v___x_418_ = lean_apply_2(v_h__3_403_, v_a_416_, v_a_417_);
return v___x_418_;
}
case 3:
{
lean_object* v_a_419_; lean_object* v_a_420_; lean_object* v___x_421_; 
lean_dec(v_h__11_411_);
lean_dec(v_h__10_410_);
lean_dec(v_h__9_409_);
lean_dec(v_h__8_408_);
lean_dec(v_h__7_407_);
lean_dec(v_h__6_406_);
lean_dec(v_h__5_405_);
lean_dec(v_h__3_403_);
lean_dec(v_h__2_402_);
lean_dec(v_h__1_401_);
v_a_419_ = lean_ctor_get(v_x_400_, 0);
lean_inc(v_a_419_);
v_a_420_ = lean_ctor_get(v_x_400_, 1);
lean_inc(v_a_420_);
lean_dec_ref_known(v_x_400_, 2);
v___x_421_ = lean_apply_2(v_h__4_404_, v_a_419_, v_a_420_);
return v___x_421_;
}
case 4:
{
lean_object* v_a_422_; lean_object* v___x_423_; 
lean_dec(v_h__11_411_);
lean_dec(v_h__10_410_);
lean_dec(v_h__9_409_);
lean_dec(v_h__8_408_);
lean_dec(v_h__7_407_);
lean_dec(v_h__6_406_);
lean_dec(v_h__4_404_);
lean_dec(v_h__3_403_);
lean_dec(v_h__2_402_);
lean_dec(v_h__1_401_);
v_a_422_ = lean_ctor_get(v_x_400_, 0);
lean_inc(v_a_422_);
lean_dec_ref_known(v_x_400_, 1);
v___x_423_ = lean_apply_1(v_h__5_405_, v_a_422_);
return v___x_423_;
}
case 5:
{
lean_object* v_a_424_; lean_object* v___x_425_; 
lean_dec(v_h__11_411_);
lean_dec(v_h__10_410_);
lean_dec(v_h__9_409_);
lean_dec(v_h__8_408_);
lean_dec(v_h__7_407_);
lean_dec(v_h__5_405_);
lean_dec(v_h__4_404_);
lean_dec(v_h__3_403_);
lean_dec(v_h__2_402_);
lean_dec(v_h__1_401_);
v_a_424_ = lean_ctor_get(v_x_400_, 0);
lean_inc(v_a_424_);
lean_dec_ref_known(v_x_400_, 1);
v___x_425_ = lean_apply_1(v_h__6_406_, v_a_424_);
return v___x_425_;
}
case 6:
{
lean_object* v_a_426_; lean_object* v___x_427_; 
lean_dec(v_h__11_411_);
lean_dec(v_h__10_410_);
lean_dec(v_h__9_409_);
lean_dec(v_h__8_408_);
lean_dec(v_h__6_406_);
lean_dec(v_h__5_405_);
lean_dec(v_h__4_404_);
lean_dec(v_h__3_403_);
lean_dec(v_h__2_402_);
lean_dec(v_h__1_401_);
v_a_426_ = lean_ctor_get(v_x_400_, 0);
lean_inc(v_a_426_);
lean_dec_ref_known(v_x_400_, 1);
v___x_427_ = lean_apply_1(v_h__7_407_, v_a_426_);
return v___x_427_;
}
case 7:
{
lean_object* v_a_428_; lean_object* v___x_429_; 
lean_dec(v_h__11_411_);
lean_dec(v_h__10_410_);
lean_dec(v_h__9_409_);
lean_dec(v_h__7_407_);
lean_dec(v_h__6_406_);
lean_dec(v_h__5_405_);
lean_dec(v_h__4_404_);
lean_dec(v_h__3_403_);
lean_dec(v_h__2_402_);
lean_dec(v_h__1_401_);
v_a_428_ = lean_ctor_get(v_x_400_, 0);
lean_inc(v_a_428_);
lean_dec_ref_known(v_x_400_, 1);
v___x_429_ = lean_apply_1(v_h__8_408_, v_a_428_);
return v___x_429_;
}
case 8:
{
lean_object* v_a_430_; lean_object* v_a_431_; lean_object* v_a_432_; lean_object* v___x_433_; 
lean_dec(v_h__11_411_);
lean_dec(v_h__10_410_);
lean_dec(v_h__8_408_);
lean_dec(v_h__7_407_);
lean_dec(v_h__6_406_);
lean_dec(v_h__5_405_);
lean_dec(v_h__4_404_);
lean_dec(v_h__3_403_);
lean_dec(v_h__2_402_);
lean_dec(v_h__1_401_);
v_a_430_ = lean_ctor_get(v_x_400_, 0);
lean_inc(v_a_430_);
v_a_431_ = lean_ctor_get(v_x_400_, 1);
lean_inc(v_a_431_);
v_a_432_ = lean_ctor_get(v_x_400_, 2);
lean_inc(v_a_432_);
lean_dec_ref_known(v_x_400_, 3);
v___x_433_ = lean_apply_3(v_h__9_409_, v_a_430_, v_a_431_, v_a_432_);
return v___x_433_;
}
case 9:
{
lean_object* v_a_434_; lean_object* v___x_435_; 
lean_dec(v_h__11_411_);
lean_dec(v_h__9_409_);
lean_dec(v_h__8_408_);
lean_dec(v_h__7_407_);
lean_dec(v_h__6_406_);
lean_dec(v_h__5_405_);
lean_dec(v_h__4_404_);
lean_dec(v_h__3_403_);
lean_dec(v_h__2_402_);
lean_dec(v_h__1_401_);
v_a_434_ = lean_ctor_get(v_x_400_, 0);
lean_inc(v_a_434_);
lean_dec_ref_known(v_x_400_, 1);
v___x_435_ = lean_apply_1(v_h__10_410_, v_a_434_);
return v___x_435_;
}
default: 
{
lean_object* v___x_436_; lean_object* v___x_437_; 
lean_dec(v_h__10_410_);
lean_dec(v_h__9_409_);
lean_dec(v_h__8_408_);
lean_dec(v_h__7_407_);
lean_dec(v_h__6_406_);
lean_dec(v_h__5_405_);
lean_dec(v_h__4_404_);
lean_dec(v_h__3_403_);
lean_dec(v_h__2_402_);
lean_dec(v_h__1_401_);
v___x_436_ = lean_box(0);
v___x_437_ = lean_apply_1(v_h__11_411_, v___x_436_);
return v___x_437_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_Translation_0__TranslationAsymmetry_LamTerm_shift_match__1_splitter(lean_object* v_motive_438_, lean_object* v_x_439_, lean_object* v_h__1_440_, lean_object* v_h__2_441_, lean_object* v_h__3_442_, lean_object* v_h__4_443_, lean_object* v_h__5_444_, lean_object* v_h__6_445_, lean_object* v_h__7_446_, lean_object* v_h__8_447_, lean_object* v_h__9_448_, lean_object* v_h__10_449_, lean_object* v_h__11_450_){
_start:
{
switch(lean_obj_tag(v_x_439_))
{
case 0:
{
lean_object* v_a_451_; lean_object* v___x_452_; 
lean_dec(v_h__11_450_);
lean_dec(v_h__10_449_);
lean_dec(v_h__9_448_);
lean_dec(v_h__8_447_);
lean_dec(v_h__7_446_);
lean_dec(v_h__6_445_);
lean_dec(v_h__5_444_);
lean_dec(v_h__4_443_);
lean_dec(v_h__3_442_);
lean_dec(v_h__2_441_);
v_a_451_ = lean_ctor_get(v_x_439_, 0);
lean_inc(v_a_451_);
lean_dec_ref_known(v_x_439_, 1);
v___x_452_ = lean_apply_1(v_h__1_440_, v_a_451_);
return v___x_452_;
}
case 1:
{
lean_object* v_a_453_; lean_object* v___x_454_; 
lean_dec(v_h__11_450_);
lean_dec(v_h__10_449_);
lean_dec(v_h__9_448_);
lean_dec(v_h__8_447_);
lean_dec(v_h__7_446_);
lean_dec(v_h__6_445_);
lean_dec(v_h__5_444_);
lean_dec(v_h__4_443_);
lean_dec(v_h__3_442_);
lean_dec(v_h__1_440_);
v_a_453_ = lean_ctor_get(v_x_439_, 0);
lean_inc(v_a_453_);
lean_dec_ref_known(v_x_439_, 1);
v___x_454_ = lean_apply_1(v_h__2_441_, v_a_453_);
return v___x_454_;
}
case 2:
{
lean_object* v_a_455_; lean_object* v_a_456_; lean_object* v___x_457_; 
lean_dec(v_h__11_450_);
lean_dec(v_h__10_449_);
lean_dec(v_h__9_448_);
lean_dec(v_h__8_447_);
lean_dec(v_h__7_446_);
lean_dec(v_h__6_445_);
lean_dec(v_h__5_444_);
lean_dec(v_h__4_443_);
lean_dec(v_h__2_441_);
lean_dec(v_h__1_440_);
v_a_455_ = lean_ctor_get(v_x_439_, 0);
lean_inc(v_a_455_);
v_a_456_ = lean_ctor_get(v_x_439_, 1);
lean_inc(v_a_456_);
lean_dec_ref_known(v_x_439_, 2);
v___x_457_ = lean_apply_2(v_h__3_442_, v_a_455_, v_a_456_);
return v___x_457_;
}
case 3:
{
lean_object* v_a_458_; lean_object* v_a_459_; lean_object* v___x_460_; 
lean_dec(v_h__11_450_);
lean_dec(v_h__10_449_);
lean_dec(v_h__9_448_);
lean_dec(v_h__8_447_);
lean_dec(v_h__7_446_);
lean_dec(v_h__6_445_);
lean_dec(v_h__5_444_);
lean_dec(v_h__3_442_);
lean_dec(v_h__2_441_);
lean_dec(v_h__1_440_);
v_a_458_ = lean_ctor_get(v_x_439_, 0);
lean_inc(v_a_458_);
v_a_459_ = lean_ctor_get(v_x_439_, 1);
lean_inc(v_a_459_);
lean_dec_ref_known(v_x_439_, 2);
v___x_460_ = lean_apply_2(v_h__4_443_, v_a_458_, v_a_459_);
return v___x_460_;
}
case 4:
{
lean_object* v_a_461_; lean_object* v___x_462_; 
lean_dec(v_h__11_450_);
lean_dec(v_h__10_449_);
lean_dec(v_h__9_448_);
lean_dec(v_h__8_447_);
lean_dec(v_h__7_446_);
lean_dec(v_h__6_445_);
lean_dec(v_h__4_443_);
lean_dec(v_h__3_442_);
lean_dec(v_h__2_441_);
lean_dec(v_h__1_440_);
v_a_461_ = lean_ctor_get(v_x_439_, 0);
lean_inc(v_a_461_);
lean_dec_ref_known(v_x_439_, 1);
v___x_462_ = lean_apply_1(v_h__5_444_, v_a_461_);
return v___x_462_;
}
case 5:
{
lean_object* v_a_463_; lean_object* v___x_464_; 
lean_dec(v_h__11_450_);
lean_dec(v_h__10_449_);
lean_dec(v_h__9_448_);
lean_dec(v_h__8_447_);
lean_dec(v_h__7_446_);
lean_dec(v_h__5_444_);
lean_dec(v_h__4_443_);
lean_dec(v_h__3_442_);
lean_dec(v_h__2_441_);
lean_dec(v_h__1_440_);
v_a_463_ = lean_ctor_get(v_x_439_, 0);
lean_inc(v_a_463_);
lean_dec_ref_known(v_x_439_, 1);
v___x_464_ = lean_apply_1(v_h__6_445_, v_a_463_);
return v___x_464_;
}
case 6:
{
lean_object* v_a_465_; lean_object* v___x_466_; 
lean_dec(v_h__11_450_);
lean_dec(v_h__10_449_);
lean_dec(v_h__9_448_);
lean_dec(v_h__8_447_);
lean_dec(v_h__6_445_);
lean_dec(v_h__5_444_);
lean_dec(v_h__4_443_);
lean_dec(v_h__3_442_);
lean_dec(v_h__2_441_);
lean_dec(v_h__1_440_);
v_a_465_ = lean_ctor_get(v_x_439_, 0);
lean_inc(v_a_465_);
lean_dec_ref_known(v_x_439_, 1);
v___x_466_ = lean_apply_1(v_h__7_446_, v_a_465_);
return v___x_466_;
}
case 7:
{
lean_object* v_a_467_; lean_object* v___x_468_; 
lean_dec(v_h__11_450_);
lean_dec(v_h__10_449_);
lean_dec(v_h__9_448_);
lean_dec(v_h__7_446_);
lean_dec(v_h__6_445_);
lean_dec(v_h__5_444_);
lean_dec(v_h__4_443_);
lean_dec(v_h__3_442_);
lean_dec(v_h__2_441_);
lean_dec(v_h__1_440_);
v_a_467_ = lean_ctor_get(v_x_439_, 0);
lean_inc(v_a_467_);
lean_dec_ref_known(v_x_439_, 1);
v___x_468_ = lean_apply_1(v_h__8_447_, v_a_467_);
return v___x_468_;
}
case 8:
{
lean_object* v_a_469_; lean_object* v_a_470_; lean_object* v_a_471_; lean_object* v___x_472_; 
lean_dec(v_h__11_450_);
lean_dec(v_h__10_449_);
lean_dec(v_h__8_447_);
lean_dec(v_h__7_446_);
lean_dec(v_h__6_445_);
lean_dec(v_h__5_444_);
lean_dec(v_h__4_443_);
lean_dec(v_h__3_442_);
lean_dec(v_h__2_441_);
lean_dec(v_h__1_440_);
v_a_469_ = lean_ctor_get(v_x_439_, 0);
lean_inc(v_a_469_);
v_a_470_ = lean_ctor_get(v_x_439_, 1);
lean_inc(v_a_470_);
v_a_471_ = lean_ctor_get(v_x_439_, 2);
lean_inc(v_a_471_);
lean_dec_ref_known(v_x_439_, 3);
v___x_472_ = lean_apply_3(v_h__9_448_, v_a_469_, v_a_470_, v_a_471_);
return v___x_472_;
}
case 9:
{
lean_object* v_a_473_; lean_object* v___x_474_; 
lean_dec(v_h__11_450_);
lean_dec(v_h__9_448_);
lean_dec(v_h__8_447_);
lean_dec(v_h__7_446_);
lean_dec(v_h__6_445_);
lean_dec(v_h__5_444_);
lean_dec(v_h__4_443_);
lean_dec(v_h__3_442_);
lean_dec(v_h__2_441_);
lean_dec(v_h__1_440_);
v_a_473_ = lean_ctor_get(v_x_439_, 0);
lean_inc(v_a_473_);
lean_dec_ref_known(v_x_439_, 1);
v___x_474_ = lean_apply_1(v_h__10_449_, v_a_473_);
return v___x_474_;
}
default: 
{
lean_object* v___x_475_; lean_object* v___x_476_; 
lean_dec(v_h__10_449_);
lean_dec(v_h__9_448_);
lean_dec(v_h__8_447_);
lean_dec(v_h__7_446_);
lean_dec(v_h__6_445_);
lean_dec(v_h__5_444_);
lean_dec(v_h__4_443_);
lean_dec(v_h__3_442_);
lean_dec(v_h__2_441_);
lean_dec(v_h__1_440_);
v___x_475_ = lean_box(0);
v___x_476_ = lean_apply_1(v_h__11_450_, v___x_475_);
return v___x_476_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_ctorIdx(lean_object* v_x_477_){
_start:
{
switch(lean_obj_tag(v_x_477_))
{
case 0:
{
lean_object* v___x_478_; 
v___x_478_ = lean_unsigned_to_nat(0u);
return v___x_478_;
}
case 1:
{
lean_object* v___x_479_; 
v___x_479_ = lean_unsigned_to_nat(1u);
return v___x_479_;
}
case 2:
{
lean_object* v___x_480_; 
v___x_480_ = lean_unsigned_to_nat(2u);
return v___x_480_;
}
case 3:
{
lean_object* v___x_481_; 
v___x_481_ = lean_unsigned_to_nat(3u);
return v___x_481_;
}
default: 
{
lean_object* v___x_482_; 
v___x_482_ = lean_unsigned_to_nat(4u);
return v___x_482_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_ctorIdx___boxed(lean_object* v_x_483_){
_start:
{
lean_object* v_res_484_; 
v_res_484_ = lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_ctorIdx(v_x_483_);
lean_dec_ref(v_x_483_);
return v_res_484_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_ctorElim___redArg(lean_object* v_t_485_, lean_object* v_k_486_){
_start:
{
switch(lean_obj_tag(v_t_485_))
{
case 0:
{
lean_object* v_a_487_; lean_object* v___x_488_; 
v_a_487_ = lean_ctor_get(v_t_485_, 0);
lean_inc(v_a_487_);
lean_dec_ref_known(v_t_485_, 1);
v___x_488_ = lean_apply_1(v_k_486_, v_a_487_);
return v___x_488_;
}
case 1:
{
lean_object* v_a_489_; lean_object* v_a_490_; lean_object* v___x_491_; 
v_a_489_ = lean_ctor_get(v_t_485_, 0);
lean_inc_ref(v_a_489_);
v_a_490_ = lean_ctor_get(v_t_485_, 1);
lean_inc_ref(v_a_490_);
lean_dec_ref_known(v_t_485_, 2);
v___x_491_ = lean_apply_2(v_k_486_, v_a_489_, v_a_490_);
return v___x_491_;
}
default: 
{
lean_object* v_a_492_; lean_object* v___x_493_; 
v_a_492_ = lean_ctor_get(v_t_485_, 0);
lean_inc_ref(v_a_492_);
lean_dec_ref(v_t_485_);
v___x_493_ = lean_apply_1(v_k_486_, v_a_492_);
return v___x_493_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_ctorElim(lean_object* v_motive_494_, lean_object* v_ctorIdx_495_, lean_object* v_t_496_, lean_object* v_h_497_, lean_object* v_k_498_){
_start:
{
lean_object* v___x_499_; 
v___x_499_ = lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_ctorElim___redArg(v_t_496_, v_k_498_);
return v___x_499_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_ctorElim___boxed(lean_object* v_motive_500_, lean_object* v_ctorIdx_501_, lean_object* v_t_502_, lean_object* v_h_503_, lean_object* v_k_504_){
_start:
{
lean_object* v_res_505_; 
v_res_505_ = lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_ctorElim(v_motive_500_, v_ctorIdx_501_, v_t_502_, v_h_503_, v_k_504_);
lean_dec(v_ctorIdx_501_);
return v_res_505_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_arr_elim___redArg(lean_object* v_t_506_, lean_object* v_arr_507_){
_start:
{
lean_object* v___x_508_; 
v___x_508_ = lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_ctorElim___redArg(v_t_506_, v_arr_507_);
return v___x_508_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_arr_elim(lean_object* v_motive_509_, lean_object* v_t_510_, lean_object* v_h_511_, lean_object* v_arr_512_){
_start:
{
lean_object* v___x_513_; 
v___x_513_ = lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_ctorElim___redArg(v_t_510_, v_arr_512_);
return v___x_513_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_comp_elim___redArg(lean_object* v_t_514_, lean_object* v_comp_515_){
_start:
{
lean_object* v___x_516_; 
v___x_516_ = lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_ctorElim___redArg(v_t_514_, v_comp_515_);
return v___x_516_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_comp_elim(lean_object* v_motive_517_, lean_object* v_t_518_, lean_object* v_h_519_, lean_object* v_comp_520_){
_start:
{
lean_object* v___x_521_; 
v___x_521_ = lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_ctorElim___redArg(v_t_518_, v_comp_520_);
return v___x_521_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_first_elim___redArg(lean_object* v_t_522_, lean_object* v_first_523_){
_start:
{
lean_object* v___x_524_; 
v___x_524_ = lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_ctorElim___redArg(v_t_522_, v_first_523_);
return v___x_524_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_first_elim(lean_object* v_motive_525_, lean_object* v_t_526_, lean_object* v_h_527_, lean_object* v_first_528_){
_start:
{
lean_object* v___x_529_; 
v___x_529_ = lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_ctorElim___redArg(v_t_526_, v_first_528_);
return v___x_529_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_left_elim___redArg(lean_object* v_t_530_, lean_object* v_left_531_){
_start:
{
lean_object* v___x_532_; 
v___x_532_ = lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_ctorElim___redArg(v_t_530_, v_left_531_);
return v___x_532_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_left_elim(lean_object* v_motive_533_, lean_object* v_t_534_, lean_object* v_h_535_, lean_object* v_left_536_){
_start:
{
lean_object* v___x_537_; 
v___x_537_ = lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_ctorElim___redArg(v_t_534_, v_left_536_);
return v___x_537_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_loop_elim___redArg(lean_object* v_t_538_, lean_object* v_loop_539_){
_start:
{
lean_object* v___x_540_; 
v___x_540_ = lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_ctorElim___redArg(v_t_538_, v_loop_539_);
return v___x_540_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_loop_elim(lean_object* v_motive_541_, lean_object* v_t_542_, lean_object* v_h_543_, lean_object* v_loop_544_){
_start:
{
lean_object* v___x_545_; 
v___x_545_ = lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_ctorElim___redArg(v_t_542_, v_loop_544_);
return v___x_545_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_arrowSize(lean_object* v_x_546_){
_start:
{
lean_object* v_f_548_; 
switch(lean_obj_tag(v_x_546_))
{
case 0:
{
lean_object* v_a_552_; lean_object* v___x_553_; lean_object* v___x_554_; lean_object* v___x_555_; 
v_a_552_ = lean_ctor_get(v_x_546_, 0);
v___x_553_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_lamSize(v_a_552_);
v___x_554_ = lean_unsigned_to_nat(1u);
v___x_555_ = lean_nat_add(v___x_553_, v___x_554_);
lean_dec(v___x_553_);
return v___x_555_;
}
case 1:
{
lean_object* v_a_556_; lean_object* v_a_557_; lean_object* v___x_558_; lean_object* v___x_559_; lean_object* v___x_560_; lean_object* v___x_561_; lean_object* v___x_562_; 
v_a_556_ = lean_ctor_get(v_x_546_, 0);
v_a_557_ = lean_ctor_get(v_x_546_, 1);
v___x_558_ = lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_arrowSize(v_a_556_);
v___x_559_ = lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_arrowSize(v_a_557_);
v___x_560_ = lean_nat_add(v___x_558_, v___x_559_);
lean_dec(v___x_559_);
lean_dec(v___x_558_);
v___x_561_ = lean_unsigned_to_nat(1u);
v___x_562_ = lean_nat_add(v___x_560_, v___x_561_);
lean_dec(v___x_560_);
return v___x_562_;
}
default: 
{
lean_object* v_a_563_; 
v_a_563_ = lean_ctor_get(v_x_546_, 0);
v_f_548_ = v_a_563_;
goto v___jp_547_;
}
}
v___jp_547_:
{
lean_object* v___x_549_; lean_object* v___x_550_; lean_object* v___x_551_; 
v___x_549_ = lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_arrowSize(v_f_548_);
v___x_550_ = lean_unsigned_to_nat(1u);
v___x_551_ = lean_nat_add(v___x_549_, v___x_550_);
lean_dec(v___x_549_);
return v___x_551_;
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_arrowSize___boxed(lean_object* v_x_564_){
_start:
{
lean_object* v_res_565_; 
v_res_565_ = lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_arrowSize(v_x_564_);
lean_dec_ref(v_x_564_);
return v_res_565_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T(lean_object* v_x_579_){
_start:
{
switch(lean_obj_tag(v_x_579_))
{
case 0:
{
lean_object* v_a_580_; 
v_a_580_ = lean_ctor_get(v_x_579_, 0);
lean_inc(v_a_580_);
lean_dec_ref_known(v_x_579_, 1);
return v_a_580_;
}
case 1:
{
lean_object* v_a_581_; lean_object* v_a_582_; lean_object* v___x_584_; uint8_t v_isShared_585_; uint8_t v_isSharedCheck_598_; 
v_a_581_ = lean_ctor_get(v_x_579_, 0);
v_a_582_ = lean_ctor_get(v_x_579_, 1);
v_isSharedCheck_598_ = !lean_is_exclusive(v_x_579_);
if (v_isSharedCheck_598_ == 0)
{
v___x_584_ = v_x_579_;
v_isShared_585_ = v_isSharedCheck_598_;
goto v_resetjp_583_;
}
else
{
lean_inc(v_a_582_);
lean_inc(v_a_581_);
lean_dec(v_x_579_);
v___x_584_ = lean_box(0);
v_isShared_585_ = v_isSharedCheck_598_;
goto v_resetjp_583_;
}
v_resetjp_583_:
{
lean_object* v___x_586_; lean_object* v___x_587_; lean_object* v___x_588_; lean_object* v___x_589_; lean_object* v___x_590_; lean_object* v___x_591_; lean_object* v___x_592_; lean_object* v___x_594_; 
v___x_586_ = lean_unsigned_to_nat(1u);
v___x_587_ = lean_unsigned_to_nat(0u);
v___x_588_ = lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T(v_a_582_);
v___x_589_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_shift(v___x_586_, v___x_587_, v___x_588_);
v___x_590_ = lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T(v_a_581_);
v___x_591_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_shift(v___x_586_, v___x_587_, v___x_590_);
v___x_592_ = ((lean_object*)(lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__0));
if (v_isShared_585_ == 0)
{
lean_ctor_set_tag(v___x_584_, 2);
lean_ctor_set(v___x_584_, 1, v___x_592_);
lean_ctor_set(v___x_584_, 0, v___x_591_);
v___x_594_ = v___x_584_;
goto v_reusejp_593_;
}
else
{
lean_object* v_reuseFailAlloc_597_; 
v_reuseFailAlloc_597_ = lean_alloc_ctor(2, 2, 0);
lean_ctor_set(v_reuseFailAlloc_597_, 0, v___x_591_);
lean_ctor_set(v_reuseFailAlloc_597_, 1, v___x_592_);
v___x_594_ = v_reuseFailAlloc_597_;
goto v_reusejp_593_;
}
v_reusejp_593_:
{
lean_object* v___x_595_; lean_object* v___x_596_; 
v___x_595_ = lean_alloc_ctor(2, 2, 0);
lean_ctor_set(v___x_595_, 0, v___x_589_);
lean_ctor_set(v___x_595_, 1, v___x_594_);
v___x_596_ = lean_alloc_ctor(1, 1, 0);
lean_ctor_set(v___x_596_, 0, v___x_595_);
return v___x_596_;
}
}
}
case 2:
{
lean_object* v_a_599_; lean_object* v___x_601_; uint8_t v_isShared_602_; uint8_t v_isSharedCheck_614_; 
v_a_599_ = lean_ctor_get(v_x_579_, 0);
v_isSharedCheck_614_ = !lean_is_exclusive(v_x_579_);
if (v_isSharedCheck_614_ == 0)
{
v___x_601_ = v_x_579_;
v_isShared_602_ = v_isSharedCheck_614_;
goto v_resetjp_600_;
}
else
{
lean_inc(v_a_599_);
lean_dec(v_x_579_);
v___x_601_ = lean_box(0);
v_isShared_602_ = v_isSharedCheck_614_;
goto v_resetjp_600_;
}
v_resetjp_600_:
{
lean_object* v___x_603_; lean_object* v___x_604_; lean_object* v___x_605_; lean_object* v___x_606_; lean_object* v___x_607_; lean_object* v___x_608_; lean_object* v___x_609_; lean_object* v___x_610_; lean_object* v___x_612_; 
v___x_603_ = lean_unsigned_to_nat(1u);
v___x_604_ = lean_unsigned_to_nat(0u);
v___x_605_ = lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T(v_a_599_);
v___x_606_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_shift(v___x_603_, v___x_604_, v___x_605_);
v___x_607_ = ((lean_object*)(lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__1));
v___x_608_ = lean_alloc_ctor(2, 2, 0);
lean_ctor_set(v___x_608_, 0, v___x_606_);
lean_ctor_set(v___x_608_, 1, v___x_607_);
v___x_609_ = ((lean_object*)(lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__2));
v___x_610_ = lean_alloc_ctor(3, 2, 0);
lean_ctor_set(v___x_610_, 0, v___x_608_);
lean_ctor_set(v___x_610_, 1, v___x_609_);
if (v_isShared_602_ == 0)
{
lean_ctor_set_tag(v___x_601_, 1);
lean_ctor_set(v___x_601_, 0, v___x_610_);
v___x_612_ = v___x_601_;
goto v_reusejp_611_;
}
else
{
lean_object* v_reuseFailAlloc_613_; 
v_reuseFailAlloc_613_ = lean_alloc_ctor(1, 1, 0);
lean_ctor_set(v_reuseFailAlloc_613_, 0, v___x_610_);
v___x_612_ = v_reuseFailAlloc_613_;
goto v_reusejp_611_;
}
v_reusejp_611_:
{
return v___x_612_;
}
}
}
case 3:
{
lean_object* v_a_615_; lean_object* v___x_617_; uint8_t v_isShared_618_; uint8_t v_isSharedCheck_631_; 
v_a_615_ = lean_ctor_get(v_x_579_, 0);
v_isSharedCheck_631_ = !lean_is_exclusive(v_x_579_);
if (v_isSharedCheck_631_ == 0)
{
v___x_617_ = v_x_579_;
v_isShared_618_ = v_isSharedCheck_631_;
goto v_resetjp_616_;
}
else
{
lean_inc(v_a_615_);
lean_dec(v_x_579_);
v___x_617_ = lean_box(0);
v_isShared_618_ = v_isSharedCheck_631_;
goto v_resetjp_616_;
}
v_resetjp_616_:
{
lean_object* v___x_619_; lean_object* v___x_620_; lean_object* v___x_621_; lean_object* v___x_622_; lean_object* v___x_623_; lean_object* v___x_624_; lean_object* v___x_626_; 
v___x_619_ = lean_unsigned_to_nat(0u);
v___x_620_ = ((lean_object*)(lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__0));
v___x_621_ = lean_unsigned_to_nat(2u);
v___x_622_ = lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T(v_a_615_);
v___x_623_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_shift(v___x_621_, v___x_619_, v___x_622_);
v___x_624_ = lean_alloc_ctor(2, 2, 0);
lean_ctor_set(v___x_624_, 0, v___x_623_);
lean_ctor_set(v___x_624_, 1, v___x_620_);
if (v_isShared_618_ == 0)
{
lean_ctor_set_tag(v___x_617_, 6);
lean_ctor_set(v___x_617_, 0, v___x_624_);
v___x_626_ = v___x_617_;
goto v_reusejp_625_;
}
else
{
lean_object* v_reuseFailAlloc_630_; 
v_reuseFailAlloc_630_ = lean_alloc_ctor(6, 1, 0);
lean_ctor_set(v_reuseFailAlloc_630_, 0, v___x_624_);
v___x_626_ = v_reuseFailAlloc_630_;
goto v_reusejp_625_;
}
v_reusejp_625_:
{
lean_object* v___x_627_; lean_object* v___x_628_; lean_object* v___x_629_; 
v___x_627_ = ((lean_object*)(lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__3));
v___x_628_ = lean_alloc_ctor(8, 3, 0);
lean_ctor_set(v___x_628_, 0, v___x_620_);
lean_ctor_set(v___x_628_, 1, v___x_626_);
lean_ctor_set(v___x_628_, 2, v___x_627_);
v___x_629_ = lean_alloc_ctor(1, 1, 0);
lean_ctor_set(v___x_629_, 0, v___x_628_);
return v___x_629_;
}
}
}
default: 
{
lean_object* v_a_632_; lean_object* v___x_634_; uint8_t v_isShared_635_; uint8_t v_isSharedCheck_648_; 
v_a_632_ = lean_ctor_get(v_x_579_, 0);
v_isSharedCheck_648_ = !lean_is_exclusive(v_x_579_);
if (v_isSharedCheck_648_ == 0)
{
v___x_634_ = v_x_579_;
v_isShared_635_ = v_isSharedCheck_648_;
goto v_resetjp_633_;
}
else
{
lean_inc(v_a_632_);
lean_dec(v_x_579_);
v___x_634_ = lean_box(0);
v_isShared_635_ = v_isSharedCheck_648_;
goto v_resetjp_633_;
}
v_resetjp_633_:
{
lean_object* v___x_636_; lean_object* v___x_637_; lean_object* v___x_638_; lean_object* v___x_639_; lean_object* v___x_640_; lean_object* v___x_641_; lean_object* v___x_643_; 
v___x_636_ = lean_unsigned_to_nat(2u);
v___x_637_ = lean_unsigned_to_nat(0u);
v___x_638_ = lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T(v_a_632_);
v___x_639_ = lp_arrow__minimality__lib_TranslationAsymmetry_LamTerm_shift(v___x_636_, v___x_637_, v___x_638_);
v___x_640_ = ((lean_object*)(lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T___closed__5));
v___x_641_ = lean_alloc_ctor(2, 2, 0);
lean_ctor_set(v___x_641_, 0, v___x_639_);
lean_ctor_set(v___x_641_, 1, v___x_640_);
if (v_isShared_635_ == 0)
{
lean_ctor_set_tag(v___x_634_, 1);
lean_ctor_set(v___x_634_, 0, v___x_641_);
v___x_643_ = v___x_634_;
goto v_reusejp_642_;
}
else
{
lean_object* v_reuseFailAlloc_647_; 
v_reuseFailAlloc_647_ = lean_alloc_ctor(1, 1, 0);
lean_ctor_set(v_reuseFailAlloc_647_, 0, v___x_641_);
v___x_643_ = v_reuseFailAlloc_647_;
goto v_reusejp_642_;
}
v_reusejp_642_:
{
lean_object* v___x_644_; lean_object* v___x_645_; lean_object* v___x_646_; 
v___x_644_ = lean_alloc_ctor(9, 1, 0);
lean_ctor_set(v___x_644_, 0, v___x_643_);
v___x_645_ = lean_alloc_ctor(4, 1, 0);
lean_ctor_set(v___x_645_, 0, v___x_644_);
v___x_646_ = lean_alloc_ctor(1, 1, 0);
lean_ctor_set(v___x_646_, 0, v___x_645_);
return v___x_646_;
}
}
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_Translation_0__TranslationAsymmetry_ArrowProg_allArrClosed_match__1_splitter___redArg(lean_object* v_x_649_, lean_object* v_h__1_650_, lean_object* v_h__2_651_, lean_object* v_h__3_652_, lean_object* v_h__4_653_, lean_object* v_h__5_654_){
_start:
{
switch(lean_obj_tag(v_x_649_))
{
case 0:
{
lean_object* v_a_655_; lean_object* v___x_656_; 
lean_dec(v_h__5_654_);
lean_dec(v_h__4_653_);
lean_dec(v_h__3_652_);
lean_dec(v_h__2_651_);
v_a_655_ = lean_ctor_get(v_x_649_, 0);
lean_inc(v_a_655_);
lean_dec_ref_known(v_x_649_, 1);
v___x_656_ = lean_apply_1(v_h__1_650_, v_a_655_);
return v___x_656_;
}
case 1:
{
lean_object* v_a_657_; lean_object* v_a_658_; lean_object* v___x_659_; 
lean_dec(v_h__5_654_);
lean_dec(v_h__4_653_);
lean_dec(v_h__3_652_);
lean_dec(v_h__1_650_);
v_a_657_ = lean_ctor_get(v_x_649_, 0);
lean_inc_ref(v_a_657_);
v_a_658_ = lean_ctor_get(v_x_649_, 1);
lean_inc_ref(v_a_658_);
lean_dec_ref_known(v_x_649_, 2);
v___x_659_ = lean_apply_2(v_h__2_651_, v_a_657_, v_a_658_);
return v___x_659_;
}
case 2:
{
lean_object* v_a_660_; lean_object* v___x_661_; 
lean_dec(v_h__5_654_);
lean_dec(v_h__4_653_);
lean_dec(v_h__2_651_);
lean_dec(v_h__1_650_);
v_a_660_ = lean_ctor_get(v_x_649_, 0);
lean_inc_ref(v_a_660_);
lean_dec_ref_known(v_x_649_, 1);
v___x_661_ = lean_apply_1(v_h__3_652_, v_a_660_);
return v___x_661_;
}
case 3:
{
lean_object* v_a_662_; lean_object* v___x_663_; 
lean_dec(v_h__5_654_);
lean_dec(v_h__3_652_);
lean_dec(v_h__2_651_);
lean_dec(v_h__1_650_);
v_a_662_ = lean_ctor_get(v_x_649_, 0);
lean_inc_ref(v_a_662_);
lean_dec_ref_known(v_x_649_, 1);
v___x_663_ = lean_apply_1(v_h__4_653_, v_a_662_);
return v___x_663_;
}
default: 
{
lean_object* v_a_664_; lean_object* v___x_665_; 
lean_dec(v_h__4_653_);
lean_dec(v_h__3_652_);
lean_dec(v_h__2_651_);
lean_dec(v_h__1_650_);
v_a_664_ = lean_ctor_get(v_x_649_, 0);
lean_inc_ref(v_a_664_);
lean_dec_ref_known(v_x_649_, 1);
v___x_665_ = lean_apply_1(v_h__5_654_, v_a_664_);
return v___x_665_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_Translation_0__TranslationAsymmetry_ArrowProg_allArrClosed_match__1_splitter(lean_object* v_motive_666_, lean_object* v_x_667_, lean_object* v_h__1_668_, lean_object* v_h__2_669_, lean_object* v_h__3_670_, lean_object* v_h__4_671_, lean_object* v_h__5_672_){
_start:
{
switch(lean_obj_tag(v_x_667_))
{
case 0:
{
lean_object* v_a_673_; lean_object* v___x_674_; 
lean_dec(v_h__5_672_);
lean_dec(v_h__4_671_);
lean_dec(v_h__3_670_);
lean_dec(v_h__2_669_);
v_a_673_ = lean_ctor_get(v_x_667_, 0);
lean_inc(v_a_673_);
lean_dec_ref_known(v_x_667_, 1);
v___x_674_ = lean_apply_1(v_h__1_668_, v_a_673_);
return v___x_674_;
}
case 1:
{
lean_object* v_a_675_; lean_object* v_a_676_; lean_object* v___x_677_; 
lean_dec(v_h__5_672_);
lean_dec(v_h__4_671_);
lean_dec(v_h__3_670_);
lean_dec(v_h__1_668_);
v_a_675_ = lean_ctor_get(v_x_667_, 0);
lean_inc_ref(v_a_675_);
v_a_676_ = lean_ctor_get(v_x_667_, 1);
lean_inc_ref(v_a_676_);
lean_dec_ref_known(v_x_667_, 2);
v___x_677_ = lean_apply_2(v_h__2_669_, v_a_675_, v_a_676_);
return v___x_677_;
}
case 2:
{
lean_object* v_a_678_; lean_object* v___x_679_; 
lean_dec(v_h__5_672_);
lean_dec(v_h__4_671_);
lean_dec(v_h__2_669_);
lean_dec(v_h__1_668_);
v_a_678_ = lean_ctor_get(v_x_667_, 0);
lean_inc_ref(v_a_678_);
lean_dec_ref_known(v_x_667_, 1);
v___x_679_ = lean_apply_1(v_h__3_670_, v_a_678_);
return v___x_679_;
}
case 3:
{
lean_object* v_a_680_; lean_object* v___x_681_; 
lean_dec(v_h__5_672_);
lean_dec(v_h__3_670_);
lean_dec(v_h__2_669_);
lean_dec(v_h__1_668_);
v_a_680_ = lean_ctor_get(v_x_667_, 0);
lean_inc_ref(v_a_680_);
lean_dec_ref_known(v_x_667_, 1);
v___x_681_ = lean_apply_1(v_h__4_671_, v_a_680_);
return v___x_681_;
}
default: 
{
lean_object* v_a_682_; lean_object* v___x_683_; 
lean_dec(v_h__4_671_);
lean_dec(v_h__3_670_);
lean_dec(v_h__2_669_);
lean_dec(v_h__1_668_);
v_a_682_ = lean_ctor_get(v_x_667_, 0);
lean_inc_ref(v_a_682_);
lean_dec_ref_known(v_x_667_, 1);
v___x_683_ = lean_apply_1(v_h__5_672_, v_a_682_);
return v___x_683_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LoopGap_loopExtLHS(lean_object* v_f_684_, lean_object* v_g_685_){
_start:
{
lean_object* v___x_686_; lean_object* v___x_687_; lean_object* v___x_688_; lean_object* v___x_689_; 
v___x_686_ = lean_alloc_ctor(2, 1, 0);
lean_ctor_set(v___x_686_, 0, v_g_685_);
v___x_687_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_687_, 0, v_f_684_);
lean_ctor_set(v___x_687_, 1, v___x_686_);
v___x_688_ = lean_alloc_ctor(4, 1, 0);
lean_ctor_set(v___x_688_, 0, v___x_687_);
v___x_689_ = lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T(v___x_688_);
return v___x_689_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LoopGap_loopExtRHS(lean_object* v_f_690_, lean_object* v_g_691_){
_start:
{
lean_object* v___x_692_; lean_object* v___x_693_; lean_object* v___x_694_; 
v___x_692_ = lean_alloc_ctor(4, 1, 0);
lean_ctor_set(v___x_692_, 0, v_f_690_);
v___x_693_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_693_, 0, v___x_692_);
lean_ctor_set(v___x_693_, 1, v_g_691_);
v___x_694_ = lp_arrow__minimality__lib_TranslationAsymmetry_ArrowProg_T(v___x_693_);
return v___x_694_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_LoopGap_interp(lean_object* v_x_697_){
_start:
{
switch(lean_obj_tag(v_x_697_))
{
case 1:
{
lean_object* v_a_698_; lean_object* v___x_700_; uint8_t v_isShared_701_; uint8_t v_isSharedCheck_706_; 
v_a_698_ = lean_ctor_get(v_x_697_, 0);
v_isSharedCheck_706_ = !lean_is_exclusive(v_x_697_);
if (v_isSharedCheck_706_ == 0)
{
v___x_700_ = v_x_697_;
v_isShared_701_ = v_isSharedCheck_706_;
goto v_resetjp_699_;
}
else
{
lean_inc(v_a_698_);
lean_dec(v_x_697_);
v___x_700_ = lean_box(0);
v_isShared_701_ = v_isSharedCheck_706_;
goto v_resetjp_699_;
}
v_resetjp_699_:
{
lean_object* v___x_702_; lean_object* v___x_704_; 
v___x_702_ = lp_arrow__minimality__lib_TranslationAsymmetry_LoopGap_interp(v_a_698_);
if (v_isShared_701_ == 0)
{
lean_ctor_set(v___x_700_, 0, v___x_702_);
v___x_704_ = v___x_700_;
goto v_reusejp_703_;
}
else
{
lean_object* v_reuseFailAlloc_705_; 
v_reuseFailAlloc_705_ = lean_alloc_ctor(1, 1, 0);
lean_ctor_set(v_reuseFailAlloc_705_, 0, v___x_702_);
v___x_704_ = v_reuseFailAlloc_705_;
goto v_reusejp_703_;
}
v_reusejp_703_:
{
return v___x_704_;
}
}
}
case 2:
{
lean_object* v_a_707_; lean_object* v_a_708_; lean_object* v___x_710_; uint8_t v_isShared_711_; uint8_t v_isSharedCheck_717_; 
v_a_707_ = lean_ctor_get(v_x_697_, 0);
v_a_708_ = lean_ctor_get(v_x_697_, 1);
v_isSharedCheck_717_ = !lean_is_exclusive(v_x_697_);
if (v_isSharedCheck_717_ == 0)
{
v___x_710_ = v_x_697_;
v_isShared_711_ = v_isSharedCheck_717_;
goto v_resetjp_709_;
}
else
{
lean_inc(v_a_708_);
lean_inc(v_a_707_);
lean_dec(v_x_697_);
v___x_710_ = lean_box(0);
v_isShared_711_ = v_isSharedCheck_717_;
goto v_resetjp_709_;
}
v_resetjp_709_:
{
lean_object* v___x_712_; lean_object* v___x_713_; lean_object* v___x_715_; 
v___x_712_ = lp_arrow__minimality__lib_TranslationAsymmetry_LoopGap_interp(v_a_707_);
v___x_713_ = lp_arrow__minimality__lib_TranslationAsymmetry_LoopGap_interp(v_a_708_);
if (v_isShared_711_ == 0)
{
lean_ctor_set(v___x_710_, 1, v___x_713_);
lean_ctor_set(v___x_710_, 0, v___x_712_);
v___x_715_ = v___x_710_;
goto v_reusejp_714_;
}
else
{
lean_object* v_reuseFailAlloc_716_; 
v_reuseFailAlloc_716_ = lean_alloc_ctor(2, 2, 0);
lean_ctor_set(v_reuseFailAlloc_716_, 0, v___x_712_);
lean_ctor_set(v_reuseFailAlloc_716_, 1, v___x_713_);
v___x_715_ = v_reuseFailAlloc_716_;
goto v_reusejp_714_;
}
v_reusejp_714_:
{
return v___x_715_;
}
}
}
case 3:
{
lean_object* v_a_718_; lean_object* v_a_719_; lean_object* v___x_721_; uint8_t v_isShared_722_; uint8_t v_isSharedCheck_728_; 
v_a_718_ = lean_ctor_get(v_x_697_, 0);
v_a_719_ = lean_ctor_get(v_x_697_, 1);
v_isSharedCheck_728_ = !lean_is_exclusive(v_x_697_);
if (v_isSharedCheck_728_ == 0)
{
v___x_721_ = v_x_697_;
v_isShared_722_ = v_isSharedCheck_728_;
goto v_resetjp_720_;
}
else
{
lean_inc(v_a_719_);
lean_inc(v_a_718_);
lean_dec(v_x_697_);
v___x_721_ = lean_box(0);
v_isShared_722_ = v_isSharedCheck_728_;
goto v_resetjp_720_;
}
v_resetjp_720_:
{
lean_object* v___x_723_; lean_object* v___x_724_; lean_object* v___x_726_; 
v___x_723_ = lp_arrow__minimality__lib_TranslationAsymmetry_LoopGap_interp(v_a_718_);
v___x_724_ = lp_arrow__minimality__lib_TranslationAsymmetry_LoopGap_interp(v_a_719_);
if (v_isShared_722_ == 0)
{
lean_ctor_set(v___x_721_, 1, v___x_724_);
lean_ctor_set(v___x_721_, 0, v___x_723_);
v___x_726_ = v___x_721_;
goto v_reusejp_725_;
}
else
{
lean_object* v_reuseFailAlloc_727_; 
v_reuseFailAlloc_727_ = lean_alloc_ctor(3, 2, 0);
lean_ctor_set(v_reuseFailAlloc_727_, 0, v___x_723_);
lean_ctor_set(v_reuseFailAlloc_727_, 1, v___x_724_);
v___x_726_ = v_reuseFailAlloc_727_;
goto v_reusejp_725_;
}
v_reusejp_725_:
{
return v___x_726_;
}
}
}
case 4:
{
lean_object* v_a_729_; lean_object* v___x_731_; uint8_t v_isShared_732_; uint8_t v_isSharedCheck_737_; 
v_a_729_ = lean_ctor_get(v_x_697_, 0);
v_isSharedCheck_737_ = !lean_is_exclusive(v_x_697_);
if (v_isSharedCheck_737_ == 0)
{
v___x_731_ = v_x_697_;
v_isShared_732_ = v_isSharedCheck_737_;
goto v_resetjp_730_;
}
else
{
lean_inc(v_a_729_);
lean_dec(v_x_697_);
v___x_731_ = lean_box(0);
v_isShared_732_ = v_isSharedCheck_737_;
goto v_resetjp_730_;
}
v_resetjp_730_:
{
lean_object* v___x_733_; lean_object* v___x_735_; 
v___x_733_ = lp_arrow__minimality__lib_TranslationAsymmetry_LoopGap_interp(v_a_729_);
if (v_isShared_732_ == 0)
{
lean_ctor_set(v___x_731_, 0, v___x_733_);
v___x_735_ = v___x_731_;
goto v_reusejp_734_;
}
else
{
lean_object* v_reuseFailAlloc_736_; 
v_reuseFailAlloc_736_ = lean_alloc_ctor(4, 1, 0);
lean_ctor_set(v_reuseFailAlloc_736_, 0, v___x_733_);
v___x_735_ = v_reuseFailAlloc_736_;
goto v_reusejp_734_;
}
v_reusejp_734_:
{
return v___x_735_;
}
}
}
case 5:
{
lean_object* v_a_738_; lean_object* v___x_740_; uint8_t v_isShared_741_; uint8_t v_isSharedCheck_746_; 
v_a_738_ = lean_ctor_get(v_x_697_, 0);
v_isSharedCheck_746_ = !lean_is_exclusive(v_x_697_);
if (v_isSharedCheck_746_ == 0)
{
v___x_740_ = v_x_697_;
v_isShared_741_ = v_isSharedCheck_746_;
goto v_resetjp_739_;
}
else
{
lean_inc(v_a_738_);
lean_dec(v_x_697_);
v___x_740_ = lean_box(0);
v_isShared_741_ = v_isSharedCheck_746_;
goto v_resetjp_739_;
}
v_resetjp_739_:
{
lean_object* v___x_742_; lean_object* v___x_744_; 
v___x_742_ = lp_arrow__minimality__lib_TranslationAsymmetry_LoopGap_interp(v_a_738_);
if (v_isShared_741_ == 0)
{
lean_ctor_set(v___x_740_, 0, v___x_742_);
v___x_744_ = v___x_740_;
goto v_reusejp_743_;
}
else
{
lean_object* v_reuseFailAlloc_745_; 
v_reuseFailAlloc_745_ = lean_alloc_ctor(5, 1, 0);
lean_ctor_set(v_reuseFailAlloc_745_, 0, v___x_742_);
v___x_744_ = v_reuseFailAlloc_745_;
goto v_reusejp_743_;
}
v_reusejp_743_:
{
return v___x_744_;
}
}
}
case 6:
{
lean_object* v_a_747_; lean_object* v___x_749_; uint8_t v_isShared_750_; uint8_t v_isSharedCheck_755_; 
v_a_747_ = lean_ctor_get(v_x_697_, 0);
v_isSharedCheck_755_ = !lean_is_exclusive(v_x_697_);
if (v_isSharedCheck_755_ == 0)
{
v___x_749_ = v_x_697_;
v_isShared_750_ = v_isSharedCheck_755_;
goto v_resetjp_748_;
}
else
{
lean_inc(v_a_747_);
lean_dec(v_x_697_);
v___x_749_ = lean_box(0);
v_isShared_750_ = v_isSharedCheck_755_;
goto v_resetjp_748_;
}
v_resetjp_748_:
{
lean_object* v___x_751_; lean_object* v___x_753_; 
v___x_751_ = lp_arrow__minimality__lib_TranslationAsymmetry_LoopGap_interp(v_a_747_);
if (v_isShared_750_ == 0)
{
lean_ctor_set(v___x_749_, 0, v___x_751_);
v___x_753_ = v___x_749_;
goto v_reusejp_752_;
}
else
{
lean_object* v_reuseFailAlloc_754_; 
v_reuseFailAlloc_754_ = lean_alloc_ctor(6, 1, 0);
lean_ctor_set(v_reuseFailAlloc_754_, 0, v___x_751_);
v___x_753_ = v_reuseFailAlloc_754_;
goto v_reusejp_752_;
}
v_reusejp_752_:
{
return v___x_753_;
}
}
}
case 7:
{
lean_object* v_a_756_; lean_object* v___x_758_; uint8_t v_isShared_759_; uint8_t v_isSharedCheck_764_; 
v_a_756_ = lean_ctor_get(v_x_697_, 0);
v_isSharedCheck_764_ = !lean_is_exclusive(v_x_697_);
if (v_isSharedCheck_764_ == 0)
{
v___x_758_ = v_x_697_;
v_isShared_759_ = v_isSharedCheck_764_;
goto v_resetjp_757_;
}
else
{
lean_inc(v_a_756_);
lean_dec(v_x_697_);
v___x_758_ = lean_box(0);
v_isShared_759_ = v_isSharedCheck_764_;
goto v_resetjp_757_;
}
v_resetjp_757_:
{
lean_object* v___x_760_; lean_object* v___x_762_; 
v___x_760_ = lp_arrow__minimality__lib_TranslationAsymmetry_LoopGap_interp(v_a_756_);
if (v_isShared_759_ == 0)
{
lean_ctor_set(v___x_758_, 0, v___x_760_);
v___x_762_ = v___x_758_;
goto v_reusejp_761_;
}
else
{
lean_object* v_reuseFailAlloc_763_; 
v_reuseFailAlloc_763_ = lean_alloc_ctor(7, 1, 0);
lean_ctor_set(v_reuseFailAlloc_763_, 0, v___x_760_);
v___x_762_ = v_reuseFailAlloc_763_;
goto v_reusejp_761_;
}
v_reusejp_761_:
{
return v___x_762_;
}
}
}
case 8:
{
lean_object* v_a_765_; lean_object* v_a_766_; lean_object* v_a_767_; lean_object* v___x_769_; uint8_t v_isShared_770_; uint8_t v_isSharedCheck_777_; 
v_a_765_ = lean_ctor_get(v_x_697_, 0);
v_a_766_ = lean_ctor_get(v_x_697_, 1);
v_a_767_ = lean_ctor_get(v_x_697_, 2);
v_isSharedCheck_777_ = !lean_is_exclusive(v_x_697_);
if (v_isSharedCheck_777_ == 0)
{
v___x_769_ = v_x_697_;
v_isShared_770_ = v_isSharedCheck_777_;
goto v_resetjp_768_;
}
else
{
lean_inc(v_a_767_);
lean_inc(v_a_766_);
lean_inc(v_a_765_);
lean_dec(v_x_697_);
v___x_769_ = lean_box(0);
v_isShared_770_ = v_isSharedCheck_777_;
goto v_resetjp_768_;
}
v_resetjp_768_:
{
lean_object* v___x_771_; lean_object* v___x_772_; lean_object* v___x_773_; lean_object* v___x_775_; 
v___x_771_ = lp_arrow__minimality__lib_TranslationAsymmetry_LoopGap_interp(v_a_765_);
v___x_772_ = lp_arrow__minimality__lib_TranslationAsymmetry_LoopGap_interp(v_a_766_);
v___x_773_ = lp_arrow__minimality__lib_TranslationAsymmetry_LoopGap_interp(v_a_767_);
if (v_isShared_770_ == 0)
{
lean_ctor_set(v___x_769_, 2, v___x_773_);
lean_ctor_set(v___x_769_, 1, v___x_772_);
lean_ctor_set(v___x_769_, 0, v___x_771_);
v___x_775_ = v___x_769_;
goto v_reusejp_774_;
}
else
{
lean_object* v_reuseFailAlloc_776_; 
v_reuseFailAlloc_776_ = lean_alloc_ctor(8, 3, 0);
lean_ctor_set(v_reuseFailAlloc_776_, 0, v___x_771_);
lean_ctor_set(v_reuseFailAlloc_776_, 1, v___x_772_);
lean_ctor_set(v_reuseFailAlloc_776_, 2, v___x_773_);
v___x_775_ = v_reuseFailAlloc_776_;
goto v_reusejp_774_;
}
v_reusejp_774_:
{
return v___x_775_;
}
}
}
case 9:
{
lean_object* v___x_778_; 
lean_dec_ref_known(v_x_697_, 1);
v___x_778_ = ((lean_object*)(lp_arrow__minimality__lib_TranslationAsymmetry_LoopGap_interp___closed__0));
return v___x_778_;
}
default: 
{
return v_x_697_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_ctorIdx(lean_object* v_x_779_){
_start:
{
switch(lean_obj_tag(v_x_779_))
{
case 0:
{
lean_object* v___x_780_; 
v___x_780_ = lean_unsigned_to_nat(0u);
return v___x_780_;
}
case 1:
{
lean_object* v___x_781_; 
v___x_781_ = lean_unsigned_to_nat(1u);
return v___x_781_;
}
case 2:
{
lean_object* v___x_782_; 
v___x_782_ = lean_unsigned_to_nat(2u);
return v___x_782_;
}
case 3:
{
lean_object* v___x_783_; 
v___x_783_ = lean_unsigned_to_nat(3u);
return v___x_783_;
}
case 4:
{
lean_object* v___x_784_; 
v___x_784_ = lean_unsigned_to_nat(4u);
return v___x_784_;
}
default: 
{
lean_object* v___x_785_; 
v___x_785_ = lean_unsigned_to_nat(5u);
return v___x_785_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_ctorIdx___boxed(lean_object* v_x_786_){
_start:
{
lean_object* v_res_787_; 
v_res_787_ = lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_ctorIdx(v_x_786_);
lean_dec(v_x_786_);
return v_res_787_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_ctorElim___redArg(lean_object* v_t_788_, lean_object* v_k_789_){
_start:
{
switch(lean_obj_tag(v_t_788_))
{
case 1:
{
lean_object* v_a_790_; lean_object* v_a_791_; lean_object* v___x_792_; 
v_a_790_ = lean_ctor_get(v_t_788_, 0);
lean_inc(v_a_790_);
v_a_791_ = lean_ctor_get(v_t_788_, 1);
lean_inc(v_a_791_);
lean_dec_ref_known(v_t_788_, 2);
v___x_792_ = lean_apply_2(v_k_789_, v_a_790_, v_a_791_);
return v___x_792_;
}
case 5:
{
return v_k_789_;
}
default: 
{
lean_object* v_a_793_; lean_object* v___x_794_; 
v_a_793_ = lean_ctor_get(v_t_788_, 0);
lean_inc(v_a_793_);
lean_dec(v_t_788_);
v___x_794_ = lean_apply_1(v_k_789_, v_a_793_);
return v___x_794_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_ctorElim(lean_object* v_motive_795_, lean_object* v_ctorIdx_796_, lean_object* v_t_797_, lean_object* v_h_798_, lean_object* v_k_799_){
_start:
{
lean_object* v___x_800_; 
v___x_800_ = lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_ctorElim___redArg(v_t_797_, v_k_799_);
return v___x_800_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_ctorElim___boxed(lean_object* v_motive_801_, lean_object* v_ctorIdx_802_, lean_object* v_t_803_, lean_object* v_h_804_, lean_object* v_k_805_){
_start:
{
lean_object* v_res_806_; 
v_res_806_ = lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_ctorElim(v_motive_801_, v_ctorIdx_802_, v_t_803_, v_h_804_, v_k_805_);
lean_dec(v_ctorIdx_802_);
return v_res_806_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_arr_elim___redArg(lean_object* v_t_807_, lean_object* v_arr_808_){
_start:
{
lean_object* v___x_809_; 
v___x_809_ = lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_ctorElim___redArg(v_t_807_, v_arr_808_);
return v___x_809_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_arr_elim(lean_object* v_motive_810_, lean_object* v_t_811_, lean_object* v_h_812_, lean_object* v_arr_813_){
_start:
{
lean_object* v___x_814_; 
v___x_814_ = lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_ctorElim___redArg(v_t_811_, v_arr_813_);
return v___x_814_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_comp_elim___redArg(lean_object* v_t_815_, lean_object* v_comp_816_){
_start:
{
lean_object* v___x_817_; 
v___x_817_ = lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_ctorElim___redArg(v_t_815_, v_comp_816_);
return v___x_817_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_comp_elim(lean_object* v_motive_818_, lean_object* v_t_819_, lean_object* v_h_820_, lean_object* v_comp_821_){
_start:
{
lean_object* v___x_822_; 
v___x_822_ = lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_ctorElim___redArg(v_t_819_, v_comp_821_);
return v___x_822_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_first_elim___redArg(lean_object* v_t_823_, lean_object* v_first_824_){
_start:
{
lean_object* v___x_825_; 
v___x_825_ = lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_ctorElim___redArg(v_t_823_, v_first_824_);
return v___x_825_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_first_elim(lean_object* v_motive_826_, lean_object* v_t_827_, lean_object* v_h_828_, lean_object* v_first_829_){
_start:
{
lean_object* v___x_830_; 
v___x_830_ = lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_ctorElim___redArg(v_t_827_, v_first_829_);
return v___x_830_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_left_elim___redArg(lean_object* v_t_831_, lean_object* v_left_832_){
_start:
{
lean_object* v___x_833_; 
v___x_833_ = lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_ctorElim___redArg(v_t_831_, v_left_832_);
return v___x_833_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_left_elim(lean_object* v_motive_834_, lean_object* v_t_835_, lean_object* v_h_836_, lean_object* v_left_837_){
_start:
{
lean_object* v___x_838_; 
v___x_838_ = lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_ctorElim___redArg(v_t_835_, v_left_837_);
return v___x_838_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_loop_elim___redArg(lean_object* v_t_839_, lean_object* v_loop_840_){
_start:
{
lean_object* v___x_841_; 
v___x_841_ = lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_ctorElim___redArg(v_t_839_, v_loop_840_);
return v___x_841_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_loop_elim(lean_object* v_motive_842_, lean_object* v_t_843_, lean_object* v_h_844_, lean_object* v_loop_845_){
_start:
{
lean_object* v___x_846_; 
v___x_846_ = lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_ctorElim___redArg(v_t_843_, v_loop_845_);
return v___x_846_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_app_elim___redArg(lean_object* v_t_847_, lean_object* v_app_848_){
_start:
{
lean_object* v___x_849_; 
v___x_849_ = lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_ctorElim___redArg(v_t_847_, v_app_848_);
return v___x_849_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_app_elim(lean_object* v_motive_850_, lean_object* v_t_851_, lean_object* v_h_852_, lean_object* v_app_853_){
_start:
{
lean_object* v___x_854_; 
v___x_854_ = lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_ArrowProgApply_ctorElim___redArg(v_t_851_, v_app_853_);
return v___x_854_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_S(lean_object* v_x_866_){
_start:
{
switch(lean_obj_tag(v_x_866_))
{
case 0:
{
lean_object* v___x_869_; 
lean_dec_ref_known(v_x_866_, 1);
v___x_869_ = lean_box(5);
return v___x_869_;
}
case 1:
{
lean_object* v___x_870_; 
lean_dec_ref_known(v_x_866_, 1);
v___x_870_ = ((lean_object*)(lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_S___closed__1));
return v___x_870_;
}
case 2:
{
lean_object* v___x_871_; 
lean_dec_ref_known(v_x_866_, 2);
v___x_871_ = ((lean_object*)(lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_S___closed__4));
return v___x_871_;
}
case 4:
{
lean_dec_ref_known(v_x_866_, 1);
goto v___jp_867_;
}
case 5:
{
lean_dec_ref_known(v_x_866_, 1);
goto v___jp_867_;
}
case 6:
{
lean_dec_ref_known(v_x_866_, 1);
goto v___jp_867_;
}
case 7:
{
lean_dec_ref_known(v_x_866_, 1);
goto v___jp_867_;
}
case 9:
{
lean_dec_ref_known(v_x_866_, 1);
goto v___jp_867_;
}
case 10:
{
lean_object* v___x_872_; 
v___x_872_ = lean_alloc_ctor(0, 1, 0);
lean_ctor_set(v___x_872_, 0, v_x_866_);
return v___x_872_;
}
default: 
{
lean_object* v___x_873_; 
lean_dec(v_x_866_);
v___x_873_ = ((lean_object*)(lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_S___closed__0));
return v___x_873_;
}
}
v___jp_867_:
{
lean_object* v___x_868_; 
v___x_868_ = ((lean_object*)(lp_arrow__minimality__lib_TranslationAsymmetry_InverseTranslation_S___closed__0));
return v___x_868_;
}
}
}
lean_object* initialize_Init(uint8_t builtin);
lean_object* initialize_Init(uint8_t builtin);
void lean_initialize_runtime_module();
static bool _G_initialized = false;
LEAN_EXPORT lean_object* initialize_arrow__minimality__lib_ArrowMinimalityLib_Translation(uint8_t builtin) {
lean_object * res;
if (_G_initialized) return lean_io_result_mk_ok(lean_box(0));
_G_initialized = true;
lean_initialize_runtime_module();
res = initialize_Init(builtin);
if (lean_io_result_is_error(res)) return res;
lean_dec_ref(res);
res = initialize_Init(builtin);
if (lean_io_result_is_error(res)) return res;
lean_dec_ref(res);
return lean_io_result_mk_ok(lean_box(0));
}
#ifdef __cplusplus
}
#endif
