// Lean compiler output
// Module: ArrowMinimalityLib.StructuralInfo
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
lean_object* l_Lean_Name_mkStr1(lean_object*);
lean_object* l_Lean_Name_mkStr2(lean_object*, lean_object*);
lean_object* l_Lean_Name_mkStr3(lean_object*, lean_object*, lean_object*);
uint8_t l_Lean_Syntax_isOfKind(lean_object*, lean_object*);
lean_object* l_Lean_Syntax_getArg(lean_object*, lean_object*);
lean_object* l_Lean_SourceInfo_fromRef(lean_object*, uint8_t);
lean_object* l_Lean_Name_mkStr4(lean_object*, lean_object*, lean_object*, lean_object*);
lean_object* l_String_toRawSubstring_x27(lean_object*);
lean_object* l_Lean_addMacroScope(lean_object*, lean_object*, lean_object*);
lean_object* l_Lean_Syntax_node2(lean_object*, lean_object*, lean_object*, lean_object*);
uint8_t l_Lean_Syntax_matchesNull(lean_object*, lean_object*);
lean_object* l_Lean_replaceRef(lean_object*, lean_object*);
lean_object* l_Lean_Syntax_node3(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorIdx___redArg(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorIdx___redArg___boxed(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorIdx(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorIdx___boxed(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorElim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorElim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorElim___boxed(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_pure_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_pure_elim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_seq_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_seq_elim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_fanout_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_fanout_elim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_choice_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_choice_elim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_op_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_op_elim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_loop_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_loop_elim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_apply_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_apply_elim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorIdx___redArg(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorIdx___redArg___boxed(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorIdx(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorIdx___boxed(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorElim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorElim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorElim___boxed(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_pure_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_pure_elim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_bind_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_bind_elim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_choice_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_choice_elim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_op_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_op_elim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_loop_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_loop_elim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorIdx___redArg(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorIdx___redArg___boxed(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorIdx(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorIdx___boxed(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorElim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorElim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorElim___boxed(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_pure_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_pure_elim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_seq_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_seq_elim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_fanout_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_fanout_elim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_choice_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_choice_elim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_op_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_op_elim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_loop_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_loop_elim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_apply_elim___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_apply_elim(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_ctorIdx(uint8_t);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_ctorIdx___boxed(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_ctorElim___redArg(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_ctorElim___redArg___boxed(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_ctorElim(lean_object*, lean_object*, uint8_t, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_ctorElim___boxed(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_fanout__independence_elim___redArg(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_fanout__independence_elim___redArg___boxed(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_fanout__independence_elim(lean_object*, uint8_t, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_fanout__independence_elim___boxed(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_branching_elim___redArg(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_branching_elim___redArg___boxed(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_branching_elim(lean_object*, uint8_t, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_branching_elim___boxed(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_per__branch__effects_elim___redArg(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_per__branch__effects_elim___redArg___boxed(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_per__branch__effects_elim(lean_object*, uint8_t, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_per__branch__effects_elim___boxed(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_dependency__graph_elim___redArg(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_dependency__graph_elim___redArg___boxed(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_dependency__graph_elim(lean_object*, uint8_t, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_dependency__graph_elim___boxed(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_feedback_elim___redArg(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_feedback_elim___redArg___boxed(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_feedback_elim(lean_object*, uint8_t, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_feedback_elim___boxed(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_effect__set_elim___redArg(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_effect__set_elim___redArg___boxed(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_effect__set_elim(lean_object*, uint8_t, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_effect__set_elim___boxed(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_op__provenance_elim___redArg(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_op__provenance_elim___redArg___boxed(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_op__provenance_elim(lean_object*, uint8_t, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_op__provenance_elim___boxed(lean_object*, lean_object*, lean_object*, lean_object*);
static const lean_string_object lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 15, .m_capacity = 15, .m_length = 14, .m_data = "StructuralInfo"};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__0 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__0_value;
static const lean_string_object lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__1_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 11, .m_capacity = 11, .m_length = 8, .m_data = "term_⊆f_"};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__1 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__1_value;
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__2_value_aux_0 = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 8, .m_other = 2, .m_tag = 1}, .m_objs = {((lean_object*)(((size_t)(0) << 1) | 1)),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__0_value),LEAN_SCALAR_PTR_LITERAL(74, 228, 189, 8, 85, 188, 209, 216)}};
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__2_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 8, .m_other = 2, .m_tag = 1}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__2_value_aux_0),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__1_value),LEAN_SCALAR_PTR_LITERAL(119, 70, 246, 92, 181, 22, 182, 30)}};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__2 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__2_value;
static const lean_string_object lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__3_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 8, .m_capacity = 8, .m_length = 7, .m_data = "andthen"};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__3 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__3_value;
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__4_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 8, .m_other = 2, .m_tag = 1}, .m_objs = {((lean_object*)(((size_t)(0) << 1) | 1)),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__3_value),LEAN_SCALAR_PTR_LITERAL(40, 255, 78, 30, 143, 119, 117, 174)}};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__4 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__4_value;
static const lean_string_object lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__5_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 7, .m_capacity = 7, .m_length = 4, .m_data = " ⊆f "};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__5 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__5_value;
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__6_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 5}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__5_value)}};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__6 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__6_value;
static const lean_string_object lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__7_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 5, .m_capacity = 5, .m_length = 4, .m_data = "term"};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__7 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__7_value;
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__8_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 8, .m_other = 2, .m_tag = 1}, .m_objs = {((lean_object*)(((size_t)(0) << 1) | 1)),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__7_value),LEAN_SCALAR_PTR_LITERAL(187, 230, 181, 162, 253, 146, 122, 119)}};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__8 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__8_value;
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__9_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 7}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__8_value),((lean_object*)(((size_t)(51) << 1) | 1))}};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__9 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__9_value;
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__10_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*3 + 0, .m_other = 3, .m_tag = 2}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__4_value),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__6_value),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__9_value)}};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__10 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__10_value;
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__11_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*4 + 0, .m_other = 4, .m_tag = 4}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__2_value),((lean_object*)(((size_t)(50) << 1) | 1)),((lean_object*)(((size_t)(51) << 1) | 1)),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__10_value)}};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__11 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__11_value;
LEAN_EXPORT const lean_object* lp_arrow__minimality__lib_StructuralInfo_term___u2286f__ = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__11_value;
static const lean_string_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 5, .m_capacity = 5, .m_length = 4, .m_data = "Lean"};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__0 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__0_value;
static const lean_string_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__1_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 7, .m_capacity = 7, .m_length = 6, .m_data = "Parser"};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__1 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__1_value;
static const lean_string_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__2_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 5, .m_capacity = 5, .m_length = 4, .m_data = "Term"};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__2 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__2_value;
static const lean_string_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__3_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 4, .m_capacity = 4, .m_length = 3, .m_data = "app"};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__3 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__3_value;
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__4_value_aux_0 = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 8, .m_other = 2, .m_tag = 1}, .m_objs = {((lean_object*)(((size_t)(0) << 1) | 1)),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__0_value),LEAN_SCALAR_PTR_LITERAL(70, 193, 83, 126, 233, 67, 208, 165)}};
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__4_value_aux_1 = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 8, .m_other = 2, .m_tag = 1}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__4_value_aux_0),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__1_value),LEAN_SCALAR_PTR_LITERAL(103, 136, 125, 166, 167, 98, 71, 111)}};
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__4_value_aux_2 = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 8, .m_other = 2, .m_tag = 1}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__4_value_aux_1),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__2_value),LEAN_SCALAR_PTR_LITERAL(75, 170, 162, 138, 136, 204, 251, 229)}};
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__4_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 8, .m_other = 2, .m_tag = 1}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__4_value_aux_2),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__3_value),LEAN_SCALAR_PTR_LITERAL(69, 118, 10, 41, 220, 156, 243, 179)}};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__4 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__4_value;
static const lean_string_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__5_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 15, .m_capacity = 15, .m_length = 14, .m_data = "FactSet.subset"};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__5 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__5_value;
static lean_once_cell_t lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__6_once = LEAN_ONCE_CELL_INITIALIZER;
static lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__6;
static const lean_string_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__7_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 8, .m_capacity = 8, .m_length = 7, .m_data = "FactSet"};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__7 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__7_value;
static const lean_string_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__8_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 7, .m_capacity = 7, .m_length = 6, .m_data = "subset"};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__8 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__8_value;
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__9_value_aux_0 = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 8, .m_other = 2, .m_tag = 1}, .m_objs = {((lean_object*)(((size_t)(0) << 1) | 1)),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__7_value),LEAN_SCALAR_PTR_LITERAL(208, 21, 7, 7, 190, 201, 134, 82)}};
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__9_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 8, .m_other = 2, .m_tag = 1}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__9_value_aux_0),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__8_value),LEAN_SCALAR_PTR_LITERAL(15, 139, 167, 123, 194, 252, 119, 143)}};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__9 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__9_value;
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__10_value_aux_0 = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 8, .m_other = 2, .m_tag = 1}, .m_objs = {((lean_object*)(((size_t)(0) << 1) | 1)),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__0_value),LEAN_SCALAR_PTR_LITERAL(74, 228, 189, 8, 85, 188, 209, 216)}};
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__10_value_aux_1 = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 8, .m_other = 2, .m_tag = 1}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__10_value_aux_0),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__7_value),LEAN_SCALAR_PTR_LITERAL(117, 169, 229, 249, 99, 37, 42, 80)}};
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__10_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 8, .m_other = 2, .m_tag = 1}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__10_value_aux_1),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__8_value),LEAN_SCALAR_PTR_LITERAL(238, 112, 41, 18, 29, 224, 107, 21)}};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__10 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__10_value;
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__11_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 1}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__10_value),((lean_object*)(((size_t)(0) << 1) | 1))}};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__11 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__11_value;
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__12_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 1}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__11_value),((lean_object*)(((size_t)(0) << 1) | 1))}};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__12 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__12_value;
static const lean_string_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__13_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 5, .m_capacity = 5, .m_length = 4, .m_data = "null"};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__13 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__13_value;
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__14_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 8, .m_other = 2, .m_tag = 1}, .m_objs = {((lean_object*)(((size_t)(0) << 1) | 1)),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__13_value),LEAN_SCALAR_PTR_LITERAL(24, 58, 49, 223, 146, 207, 197, 136)}};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__14 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__14_value;
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___boxed(lean_object*, lean_object*, lean_object*);
static const lean_string_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______unexpand__StructuralInfo__FactSet__subset__1___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 6, .m_capacity = 6, .m_length = 5, .m_data = "ident"};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______unexpand__StructuralInfo__FactSet__subset__1___closed__0 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______unexpand__StructuralInfo__FactSet__subset__1___closed__0_value;
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______unexpand__StructuralInfo__FactSet__subset__1___closed__1_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 8, .m_other = 2, .m_tag = 1}, .m_objs = {((lean_object*)(((size_t)(0) << 1) | 1)),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______unexpand__StructuralInfo__FactSet__subset__1___closed__0_value),LEAN_SCALAR_PTR_LITERAL(52, 159, 208, 51, 14, 60, 6, 71)}};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______unexpand__StructuralInfo__FactSet__subset__1___closed__1 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______unexpand__StructuralInfo__FactSet__subset__1___closed__1_value;
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______unexpand__StructuralInfo__FactSet__subset__1(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______unexpand__StructuralInfo__FactSet__subset__1___boxed(lean_object*, lean_object*, lean_object*);
static const lean_string_object lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 11, .m_capacity = 11, .m_length = 8, .m_data = "term_∈f_"};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__0 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__0_value;
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__1_value_aux_0 = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 8, .m_other = 2, .m_tag = 1}, .m_objs = {((lean_object*)(((size_t)(0) << 1) | 1)),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__0_value),LEAN_SCALAR_PTR_LITERAL(74, 228, 189, 8, 85, 188, 209, 216)}};
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__1_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 8, .m_other = 2, .m_tag = 1}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__1_value_aux_0),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__0_value),LEAN_SCALAR_PTR_LITERAL(43, 126, 147, 29, 3, 176, 90, 87)}};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__1 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__1_value;
static const lean_string_object lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__2_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 7, .m_capacity = 7, .m_length = 4, .m_data = " ∈f "};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__2 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__2_value;
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__3_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*1 + 0, .m_other = 1, .m_tag = 5}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__2_value)}};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__3 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__3_value;
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__4_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 7}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__8_value),((lean_object*)(((size_t)(56) << 1) | 1))}};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__4 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__4_value;
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__5_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*3 + 0, .m_other = 3, .m_tag = 2}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__4_value),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__3_value),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__4_value)}};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__5 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__5_value;
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__6_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*4 + 0, .m_other = 4, .m_tag = 4}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__1_value),((lean_object*)(((size_t)(55) << 1) | 1)),((lean_object*)(((size_t)(56) << 1) | 1)),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__5_value)}};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__6 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__6_value;
LEAN_EXPORT const lean_object* lp_arrow__minimality__lib_StructuralInfo_term___u2208f__ = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__6_value;
static const lean_string_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__0_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 12, .m_capacity = 12, .m_length = 11, .m_data = "FactSet.mem"};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__0 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__0_value;
static lean_once_cell_t lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__1_once = LEAN_ONCE_CELL_INITIALIZER;
static lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__1;
static const lean_string_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__2_value = {.m_header = {.m_rc = 0, .m_cs_sz = 0, .m_other = 0, .m_tag = 249}, .m_size = 4, .m_capacity = 4, .m_length = 3, .m_data = "mem"};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__2 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__2_value;
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__3_value_aux_0 = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 8, .m_other = 2, .m_tag = 1}, .m_objs = {((lean_object*)(((size_t)(0) << 1) | 1)),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__7_value),LEAN_SCALAR_PTR_LITERAL(208, 21, 7, 7, 190, 201, 134, 82)}};
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__3_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 8, .m_other = 2, .m_tag = 1}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__3_value_aux_0),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__2_value),LEAN_SCALAR_PTR_LITERAL(193, 100, 158, 173, 242, 134, 43, 53)}};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__3 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__3_value;
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__4_value_aux_0 = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 8, .m_other = 2, .m_tag = 1}, .m_objs = {((lean_object*)(((size_t)(0) << 1) | 1)),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__0_value),LEAN_SCALAR_PTR_LITERAL(74, 228, 189, 8, 85, 188, 209, 216)}};
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__4_value_aux_1 = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 8, .m_other = 2, .m_tag = 1}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__4_value_aux_0),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__7_value),LEAN_SCALAR_PTR_LITERAL(117, 169, 229, 249, 99, 37, 42, 80)}};
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__4_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 8, .m_other = 2, .m_tag = 1}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__4_value_aux_1),((lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__2_value),LEAN_SCALAR_PTR_LITERAL(184, 168, 138, 233, 113, 212, 163, 87)}};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__4 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__4_value;
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__5_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 1}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__4_value),((lean_object*)(((size_t)(0) << 1) | 1))}};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__5 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__5_value;
static const lean_ctor_object lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__6_value = {.m_header = {.m_rc = 0, .m_cs_sz = sizeof(lean_ctor_object) + sizeof(void*)*2 + 0, .m_other = 2, .m_tag = 1}, .m_objs = {((lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__5_value),((lean_object*)(((size_t)(0) << 1) | 1))}};
static const lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__6 = (const lean_object*)&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__6_value;
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___boxed(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______unexpand__StructuralInfo__FactSet__mem__1(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______unexpand__StructuralInfo__FactSet__mem__1___boxed(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_arrow__minimality__lib_StructuralInfo_containsFanout___redArg(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_containsFanout___redArg___boxed(lean_object*);
LEAN_EXPORT uint8_t lp_arrow__minimality__lib_StructuralInfo_containsFanout(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_containsFanout___boxed(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_arrow__minimality__lib_StructuralInfo_containsChoice___redArg(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_containsChoice___redArg___boxed(lean_object*);
LEAN_EXPORT uint8_t lp_arrow__minimality__lib_StructuralInfo_containsChoice(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_containsChoice___boxed(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_arrow__minimality__lib_StructuralInfo_containsLoop___redArg(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_containsLoop___redArg___boxed(lean_object*);
LEAN_EXPORT uint8_t lp_arrow__minimality__lib_StructuralInfo_containsLoop(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_containsLoop___boxed(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_arrow__minimality__lib_StructuralInfo_containsOp___redArg(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_containsOp___redArg___boxed(lean_object*);
LEAN_EXPORT uint8_t lp_arrow__minimality__lib_StructuralInfo_containsOp(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_containsOp___boxed(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_arrow__minimality__lib_StructuralInfo_holds___redArg(uint8_t, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_holds___redArg___boxed(lean_object*, lean_object*);
LEAN_EXPORT uint8_t lp_arrow__minimality__lib_StructuralInfo_holds(lean_object*, lean_object*, uint8_t, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_holds___boxed(lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_encodeMonad___redArg(lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_encodeMonad(lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_encodeTagged___redArg(lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_encodeTagged(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_StructuralInfo_0__StructuralInfo_holds_match__1_splitter___redArg(uint8_t, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_StructuralInfo_0__StructuralInfo_holds_match__1_splitter___redArg___boxed(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_StructuralInfo_0__StructuralInfo_holds_match__1_splitter(lean_object*, lean_object*, lean_object*, uint8_t, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_StructuralInfo_0__StructuralInfo_holds_match__1_splitter___boxed(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_StructuralInfo_0__StructuralInfo_containsFanout_match__1_splitter___redArg(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_StructuralInfo_0__StructuralInfo_containsFanout_match__1_splitter(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_StructuralInfo_0__StructuralInfo_containsChoice_match__1_splitter___redArg(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_StructuralInfo_0__StructuralInfo_containsChoice_match__1_splitter(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_StructuralInfo_0__StructuralInfo_containsLoop_match__1_splitter___redArg(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_StructuralInfo_0__StructuralInfo_containsLoop_match__1_splitter(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_StructuralInfo_0__StructuralInfo_containsOp_match__1_splitter___redArg(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_StructuralInfo_0__StructuralInfo_containsOp_match__1_splitter(lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*, lean_object*);
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorIdx___redArg(lean_object* v_x_1_){
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
default: 
{
lean_object* v___x_8_; 
v___x_8_ = lean_unsigned_to_nat(6u);
return v___x_8_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorIdx___redArg___boxed(lean_object* v_x_9_){
_start:
{
lean_object* v_res_10_; 
v_res_10_ = lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorIdx___redArg(v_x_9_);
lean_dec_ref(v_x_9_);
return v_res_10_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorIdx(lean_object* v_Op_11_, lean_object* v_V_12_, lean_object* v_x_13_){
_start:
{
lean_object* v___x_14_; 
v___x_14_ = lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorIdx___redArg(v_x_13_);
return v___x_14_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorIdx___boxed(lean_object* v_Op_15_, lean_object* v_V_16_, lean_object* v_x_17_){
_start:
{
lean_object* v_res_18_; 
v_res_18_ = lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorIdx(v_Op_15_, v_V_16_, v_x_17_);
lean_dec_ref(v_x_17_);
return v_res_18_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorElim___redArg(lean_object* v_t_19_, lean_object* v_k_20_){
_start:
{
switch(lean_obj_tag(v_t_19_))
{
case 0:
{
lean_object* v_v_21_; lean_object* v___x_22_; 
v_v_21_ = lean_ctor_get(v_t_19_, 0);
lean_inc(v_v_21_);
lean_dec_ref_known(v_t_19_, 1);
v___x_22_ = lean_apply_1(v_k_20_, v_v_21_);
return v___x_22_;
}
case 4:
{
lean_object* v_o_23_; lean_object* v_k_24_; lean_object* v___x_25_; 
v_o_23_ = lean_ctor_get(v_t_19_, 0);
lean_inc(v_o_23_);
v_k_24_ = lean_ctor_get(v_t_19_, 1);
lean_inc_ref(v_k_24_);
lean_dec_ref_known(v_t_19_, 2);
v___x_25_ = lean_apply_2(v_k_20_, v_o_23_, v_k_24_);
return v___x_25_;
}
case 5:
{
lean_object* v_f_26_; lean_object* v___x_27_; 
v_f_26_ = lean_ctor_get(v_t_19_, 0);
lean_inc_ref(v_f_26_);
lean_dec_ref_known(v_t_19_, 1);
v___x_27_ = lean_apply_1(v_k_20_, v_f_26_);
return v___x_27_;
}
case 6:
{
lean_object* v_f_28_; lean_object* v___x_29_; 
v_f_28_ = lean_ctor_get(v_t_19_, 0);
lean_inc_ref(v_f_28_);
lean_dec_ref_known(v_t_19_, 1);
v___x_29_ = lean_apply_1(v_k_20_, v_f_28_);
return v___x_29_;
}
default: 
{
lean_object* v_f_30_; lean_object* v_g_31_; lean_object* v___x_32_; 
v_f_30_ = lean_ctor_get(v_t_19_, 0);
lean_inc_ref(v_f_30_);
v_g_31_ = lean_ctor_get(v_t_19_, 1);
lean_inc_ref(v_g_31_);
lean_dec_ref(v_t_19_);
v___x_32_ = lean_apply_2(v_k_20_, v_f_30_, v_g_31_);
return v___x_32_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorElim(lean_object* v_Op_33_, lean_object* v_V_34_, lean_object* v_motive_35_, lean_object* v_ctorIdx_36_, lean_object* v_t_37_, lean_object* v_h_38_, lean_object* v_k_39_){
_start:
{
lean_object* v___x_40_; 
v___x_40_ = lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorElim___redArg(v_t_37_, v_k_39_);
return v___x_40_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorElim___boxed(lean_object* v_Op_41_, lean_object* v_V_42_, lean_object* v_motive_43_, lean_object* v_ctorIdx_44_, lean_object* v_t_45_, lean_object* v_h_46_, lean_object* v_k_47_){
_start:
{
lean_object* v_res_48_; 
v_res_48_ = lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorElim(v_Op_41_, v_V_42_, v_motive_43_, v_ctorIdx_44_, v_t_45_, v_h_46_, v_k_47_);
lean_dec(v_ctorIdx_44_);
return v_res_48_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_pure_elim___redArg(lean_object* v_t_49_, lean_object* v_pure_50_){
_start:
{
lean_object* v___x_51_; 
v___x_51_ = lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorElim___redArg(v_t_49_, v_pure_50_);
return v___x_51_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_pure_elim(lean_object* v_Op_52_, lean_object* v_V_53_, lean_object* v_motive_54_, lean_object* v_t_55_, lean_object* v_h_56_, lean_object* v_pure_57_){
_start:
{
lean_object* v___x_58_; 
v___x_58_ = lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorElim___redArg(v_t_55_, v_pure_57_);
return v___x_58_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_seq_elim___redArg(lean_object* v_t_59_, lean_object* v_seq_60_){
_start:
{
lean_object* v___x_61_; 
v___x_61_ = lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorElim___redArg(v_t_59_, v_seq_60_);
return v___x_61_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_seq_elim(lean_object* v_Op_62_, lean_object* v_V_63_, lean_object* v_motive_64_, lean_object* v_t_65_, lean_object* v_h_66_, lean_object* v_seq_67_){
_start:
{
lean_object* v___x_68_; 
v___x_68_ = lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorElim___redArg(v_t_65_, v_seq_67_);
return v___x_68_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_fanout_elim___redArg(lean_object* v_t_69_, lean_object* v_fanout_70_){
_start:
{
lean_object* v___x_71_; 
v___x_71_ = lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorElim___redArg(v_t_69_, v_fanout_70_);
return v___x_71_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_fanout_elim(lean_object* v_Op_72_, lean_object* v_V_73_, lean_object* v_motive_74_, lean_object* v_t_75_, lean_object* v_h_76_, lean_object* v_fanout_77_){
_start:
{
lean_object* v___x_78_; 
v___x_78_ = lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorElim___redArg(v_t_75_, v_fanout_77_);
return v___x_78_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_choice_elim___redArg(lean_object* v_t_79_, lean_object* v_choice_80_){
_start:
{
lean_object* v___x_81_; 
v___x_81_ = lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorElim___redArg(v_t_79_, v_choice_80_);
return v___x_81_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_choice_elim(lean_object* v_Op_82_, lean_object* v_V_83_, lean_object* v_motive_84_, lean_object* v_t_85_, lean_object* v_h_86_, lean_object* v_choice_87_){
_start:
{
lean_object* v___x_88_; 
v___x_88_ = lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorElim___redArg(v_t_85_, v_choice_87_);
return v___x_88_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_op_elim___redArg(lean_object* v_t_89_, lean_object* v_op_90_){
_start:
{
lean_object* v___x_91_; 
v___x_91_ = lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorElim___redArg(v_t_89_, v_op_90_);
return v___x_91_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_op_elim(lean_object* v_Op_92_, lean_object* v_V_93_, lean_object* v_motive_94_, lean_object* v_t_95_, lean_object* v_h_96_, lean_object* v_op_97_){
_start:
{
lean_object* v___x_98_; 
v___x_98_ = lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorElim___redArg(v_t_95_, v_op_97_);
return v___x_98_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_loop_elim___redArg(lean_object* v_t_99_, lean_object* v_loop_100_){
_start:
{
lean_object* v___x_101_; 
v___x_101_ = lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorElim___redArg(v_t_99_, v_loop_100_);
return v___x_101_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_loop_elim(lean_object* v_Op_102_, lean_object* v_V_103_, lean_object* v_motive_104_, lean_object* v_t_105_, lean_object* v_h_106_, lean_object* v_loop_107_){
_start:
{
lean_object* v___x_108_; 
v___x_108_ = lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorElim___redArg(v_t_105_, v_loop_107_);
return v___x_108_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_apply_elim___redArg(lean_object* v_t_109_, lean_object* v_apply_110_){
_start:
{
lean_object* v___x_111_; 
v___x_111_ = lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorElim___redArg(v_t_109_, v_apply_110_);
return v___x_111_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerArrow_apply_elim(lean_object* v_Op_112_, lean_object* v_V_113_, lean_object* v_motive_114_, lean_object* v_t_115_, lean_object* v_h_116_, lean_object* v_apply_117_){
_start:
{
lean_object* v___x_118_; 
v___x_118_ = lp_arrow__minimality__lib_StructuralInfo_FreerArrow_ctorElim___redArg(v_t_115_, v_apply_117_);
return v___x_118_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorIdx___redArg(lean_object* v_x_119_){
_start:
{
switch(lean_obj_tag(v_x_119_))
{
case 0:
{
lean_object* v___x_120_; 
v___x_120_ = lean_unsigned_to_nat(0u);
return v___x_120_;
}
case 1:
{
lean_object* v___x_121_; 
v___x_121_ = lean_unsigned_to_nat(1u);
return v___x_121_;
}
case 2:
{
lean_object* v___x_122_; 
v___x_122_ = lean_unsigned_to_nat(2u);
return v___x_122_;
}
case 3:
{
lean_object* v___x_123_; 
v___x_123_ = lean_unsigned_to_nat(3u);
return v___x_123_;
}
default: 
{
lean_object* v___x_124_; 
v___x_124_ = lean_unsigned_to_nat(4u);
return v___x_124_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorIdx___redArg___boxed(lean_object* v_x_125_){
_start:
{
lean_object* v_res_126_; 
v_res_126_ = lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorIdx___redArg(v_x_125_);
lean_dec_ref(v_x_125_);
return v_res_126_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorIdx(lean_object* v_Op_127_, lean_object* v_V_128_, lean_object* v_x_129_){
_start:
{
lean_object* v___x_130_; 
v___x_130_ = lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorIdx___redArg(v_x_129_);
return v___x_130_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorIdx___boxed(lean_object* v_Op_131_, lean_object* v_V_132_, lean_object* v_x_133_){
_start:
{
lean_object* v_res_134_; 
v_res_134_ = lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorIdx(v_Op_131_, v_V_132_, v_x_133_);
lean_dec_ref(v_x_133_);
return v_res_134_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorElim___redArg(lean_object* v_t_135_, lean_object* v_k_136_){
_start:
{
switch(lean_obj_tag(v_t_135_))
{
case 0:
{
lean_object* v_v_137_; lean_object* v___x_138_; 
v_v_137_ = lean_ctor_get(v_t_135_, 0);
lean_inc(v_v_137_);
lean_dec_ref_known(v_t_135_, 1);
v___x_138_ = lean_apply_1(v_k_136_, v_v_137_);
return v___x_138_;
}
case 3:
{
lean_object* v_o_139_; lean_object* v_k_140_; lean_object* v___x_141_; 
v_o_139_ = lean_ctor_get(v_t_135_, 0);
lean_inc(v_o_139_);
v_k_140_ = lean_ctor_get(v_t_135_, 1);
lean_inc_ref(v_k_140_);
lean_dec_ref_known(v_t_135_, 2);
v___x_141_ = lean_apply_2(v_k_136_, v_o_139_, v_k_140_);
return v___x_141_;
}
case 4:
{
lean_object* v_f_142_; lean_object* v___x_143_; 
v_f_142_ = lean_ctor_get(v_t_135_, 0);
lean_inc_ref(v_f_142_);
lean_dec_ref_known(v_t_135_, 1);
v___x_143_ = lean_apply_1(v_k_136_, v_f_142_);
return v___x_143_;
}
default: 
{
lean_object* v_m_144_; lean_object* v_k_145_; lean_object* v___x_146_; 
v_m_144_ = lean_ctor_get(v_t_135_, 0);
lean_inc_ref(v_m_144_);
v_k_145_ = lean_ctor_get(v_t_135_, 1);
lean_inc_ref(v_k_145_);
lean_dec_ref(v_t_135_);
v___x_146_ = lean_apply_2(v_k_136_, v_m_144_, v_k_145_);
return v___x_146_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorElim(lean_object* v_Op_147_, lean_object* v_V_148_, lean_object* v_motive_149_, lean_object* v_ctorIdx_150_, lean_object* v_t_151_, lean_object* v_h_152_, lean_object* v_k_153_){
_start:
{
lean_object* v___x_154_; 
v___x_154_ = lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorElim___redArg(v_t_151_, v_k_153_);
return v___x_154_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorElim___boxed(lean_object* v_Op_155_, lean_object* v_V_156_, lean_object* v_motive_157_, lean_object* v_ctorIdx_158_, lean_object* v_t_159_, lean_object* v_h_160_, lean_object* v_k_161_){
_start:
{
lean_object* v_res_162_; 
v_res_162_ = lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorElim(v_Op_155_, v_V_156_, v_motive_157_, v_ctorIdx_158_, v_t_159_, v_h_160_, v_k_161_);
lean_dec(v_ctorIdx_158_);
return v_res_162_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_pure_elim___redArg(lean_object* v_t_163_, lean_object* v_pure_164_){
_start:
{
lean_object* v___x_165_; 
v___x_165_ = lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorElim___redArg(v_t_163_, v_pure_164_);
return v___x_165_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_pure_elim(lean_object* v_Op_166_, lean_object* v_V_167_, lean_object* v_motive_168_, lean_object* v_t_169_, lean_object* v_h_170_, lean_object* v_pure_171_){
_start:
{
lean_object* v___x_172_; 
v___x_172_ = lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorElim___redArg(v_t_169_, v_pure_171_);
return v___x_172_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_bind_elim___redArg(lean_object* v_t_173_, lean_object* v_bind_174_){
_start:
{
lean_object* v___x_175_; 
v___x_175_ = lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorElim___redArg(v_t_173_, v_bind_174_);
return v___x_175_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_bind_elim(lean_object* v_Op_176_, lean_object* v_V_177_, lean_object* v_motive_178_, lean_object* v_t_179_, lean_object* v_h_180_, lean_object* v_bind_181_){
_start:
{
lean_object* v___x_182_; 
v___x_182_ = lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorElim___redArg(v_t_179_, v_bind_181_);
return v___x_182_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_choice_elim___redArg(lean_object* v_t_183_, lean_object* v_choice_184_){
_start:
{
lean_object* v___x_185_; 
v___x_185_ = lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorElim___redArg(v_t_183_, v_choice_184_);
return v___x_185_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_choice_elim(lean_object* v_Op_186_, lean_object* v_V_187_, lean_object* v_motive_188_, lean_object* v_t_189_, lean_object* v_h_190_, lean_object* v_choice_191_){
_start:
{
lean_object* v___x_192_; 
v___x_192_ = lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorElim___redArg(v_t_189_, v_choice_191_);
return v___x_192_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_op_elim___redArg(lean_object* v_t_193_, lean_object* v_op_194_){
_start:
{
lean_object* v___x_195_; 
v___x_195_ = lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorElim___redArg(v_t_193_, v_op_194_);
return v___x_195_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_op_elim(lean_object* v_Op_196_, lean_object* v_V_197_, lean_object* v_motive_198_, lean_object* v_t_199_, lean_object* v_h_200_, lean_object* v_op_201_){
_start:
{
lean_object* v___x_202_; 
v___x_202_ = lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorElim___redArg(v_t_199_, v_op_201_);
return v___x_202_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_loop_elim___redArg(lean_object* v_t_203_, lean_object* v_loop_204_){
_start:
{
lean_object* v___x_205_; 
v___x_205_ = lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorElim___redArg(v_t_203_, v_loop_204_);
return v___x_205_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_FreerMonad_loop_elim(lean_object* v_Op_206_, lean_object* v_V_207_, lean_object* v_motive_208_, lean_object* v_t_209_, lean_object* v_h_210_, lean_object* v_loop_211_){
_start:
{
lean_object* v___x_212_; 
v___x_212_ = lp_arrow__minimality__lib_StructuralInfo_FreerMonad_ctorElim___redArg(v_t_209_, v_loop_211_);
return v___x_212_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorIdx___redArg(lean_object* v_x_213_){
_start:
{
switch(lean_obj_tag(v_x_213_))
{
case 0:
{
lean_object* v___x_214_; 
v___x_214_ = lean_unsigned_to_nat(0u);
return v___x_214_;
}
case 1:
{
lean_object* v___x_215_; 
v___x_215_ = lean_unsigned_to_nat(1u);
return v___x_215_;
}
case 2:
{
lean_object* v___x_216_; 
v___x_216_ = lean_unsigned_to_nat(2u);
return v___x_216_;
}
case 3:
{
lean_object* v___x_217_; 
v___x_217_ = lean_unsigned_to_nat(3u);
return v___x_217_;
}
case 4:
{
lean_object* v___x_218_; 
v___x_218_ = lean_unsigned_to_nat(4u);
return v___x_218_;
}
case 5:
{
lean_object* v___x_219_; 
v___x_219_ = lean_unsigned_to_nat(5u);
return v___x_219_;
}
default: 
{
lean_object* v___x_220_; 
v___x_220_ = lean_unsigned_to_nat(6u);
return v___x_220_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorIdx___redArg___boxed(lean_object* v_x_221_){
_start:
{
lean_object* v_res_222_; 
v_res_222_ = lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorIdx___redArg(v_x_221_);
lean_dec_ref(v_x_221_);
return v_res_222_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorIdx(lean_object* v_Op_223_, lean_object* v_V_224_, lean_object* v_Label_225_, lean_object* v_x_226_){
_start:
{
lean_object* v___x_227_; 
v___x_227_ = lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorIdx___redArg(v_x_226_);
return v___x_227_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorIdx___boxed(lean_object* v_Op_228_, lean_object* v_V_229_, lean_object* v_Label_230_, lean_object* v_x_231_){
_start:
{
lean_object* v_res_232_; 
v_res_232_ = lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorIdx(v_Op_228_, v_V_229_, v_Label_230_, v_x_231_);
lean_dec_ref(v_x_231_);
return v_res_232_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorElim___redArg(lean_object* v_t_233_, lean_object* v_k_234_){
_start:
{
switch(lean_obj_tag(v_t_233_))
{
case 0:
{
lean_object* v_v_235_; lean_object* v___x_236_; 
v_v_235_ = lean_ctor_get(v_t_233_, 0);
lean_inc(v_v_235_);
lean_dec_ref_known(v_t_233_, 1);
v___x_236_ = lean_apply_1(v_k_234_, v_v_235_);
return v___x_236_;
}
case 4:
{
lean_object* v_label_237_; lean_object* v_o_238_; lean_object* v_k_239_; lean_object* v___x_240_; 
v_label_237_ = lean_ctor_get(v_t_233_, 0);
lean_inc(v_label_237_);
v_o_238_ = lean_ctor_get(v_t_233_, 1);
lean_inc(v_o_238_);
v_k_239_ = lean_ctor_get(v_t_233_, 2);
lean_inc_ref(v_k_239_);
lean_dec_ref_known(v_t_233_, 3);
v___x_240_ = lean_apply_3(v_k_234_, v_label_237_, v_o_238_, v_k_239_);
return v___x_240_;
}
case 5:
{
lean_object* v_f_241_; lean_object* v___x_242_; 
v_f_241_ = lean_ctor_get(v_t_233_, 0);
lean_inc_ref(v_f_241_);
lean_dec_ref_known(v_t_233_, 1);
v___x_242_ = lean_apply_1(v_k_234_, v_f_241_);
return v___x_242_;
}
case 6:
{
lean_object* v_f_243_; lean_object* v___x_244_; 
v_f_243_ = lean_ctor_get(v_t_233_, 0);
lean_inc_ref(v_f_243_);
lean_dec_ref_known(v_t_233_, 1);
v___x_244_ = lean_apply_1(v_k_234_, v_f_243_);
return v___x_244_;
}
default: 
{
lean_object* v_f_245_; lean_object* v_g_246_; lean_object* v___x_247_; 
v_f_245_ = lean_ctor_get(v_t_233_, 0);
lean_inc_ref(v_f_245_);
v_g_246_ = lean_ctor_get(v_t_233_, 1);
lean_inc_ref(v_g_246_);
lean_dec_ref(v_t_233_);
v___x_247_ = lean_apply_2(v_k_234_, v_f_245_, v_g_246_);
return v___x_247_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorElim(lean_object* v_Op_248_, lean_object* v_V_249_, lean_object* v_Label_250_, lean_object* v_motive_251_, lean_object* v_ctorIdx_252_, lean_object* v_t_253_, lean_object* v_h_254_, lean_object* v_k_255_){
_start:
{
lean_object* v___x_256_; 
v___x_256_ = lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorElim___redArg(v_t_253_, v_k_255_);
return v___x_256_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorElim___boxed(lean_object* v_Op_257_, lean_object* v_V_258_, lean_object* v_Label_259_, lean_object* v_motive_260_, lean_object* v_ctorIdx_261_, lean_object* v_t_262_, lean_object* v_h_263_, lean_object* v_k_264_){
_start:
{
lean_object* v_res_265_; 
v_res_265_ = lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorElim(v_Op_257_, v_V_258_, v_Label_259_, v_motive_260_, v_ctorIdx_261_, v_t_262_, v_h_263_, v_k_264_);
lean_dec(v_ctorIdx_261_);
return v_res_265_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_pure_elim___redArg(lean_object* v_t_266_, lean_object* v_pure_267_){
_start:
{
lean_object* v___x_268_; 
v___x_268_ = lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorElim___redArg(v_t_266_, v_pure_267_);
return v___x_268_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_pure_elim(lean_object* v_Op_269_, lean_object* v_V_270_, lean_object* v_Label_271_, lean_object* v_motive_272_, lean_object* v_t_273_, lean_object* v_h_274_, lean_object* v_pure_275_){
_start:
{
lean_object* v___x_276_; 
v___x_276_ = lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorElim___redArg(v_t_273_, v_pure_275_);
return v___x_276_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_seq_elim___redArg(lean_object* v_t_277_, lean_object* v_seq_278_){
_start:
{
lean_object* v___x_279_; 
v___x_279_ = lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorElim___redArg(v_t_277_, v_seq_278_);
return v___x_279_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_seq_elim(lean_object* v_Op_280_, lean_object* v_V_281_, lean_object* v_Label_282_, lean_object* v_motive_283_, lean_object* v_t_284_, lean_object* v_h_285_, lean_object* v_seq_286_){
_start:
{
lean_object* v___x_287_; 
v___x_287_ = lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorElim___redArg(v_t_284_, v_seq_286_);
return v___x_287_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_fanout_elim___redArg(lean_object* v_t_288_, lean_object* v_fanout_289_){
_start:
{
lean_object* v___x_290_; 
v___x_290_ = lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorElim___redArg(v_t_288_, v_fanout_289_);
return v___x_290_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_fanout_elim(lean_object* v_Op_291_, lean_object* v_V_292_, lean_object* v_Label_293_, lean_object* v_motive_294_, lean_object* v_t_295_, lean_object* v_h_296_, lean_object* v_fanout_297_){
_start:
{
lean_object* v___x_298_; 
v___x_298_ = lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorElim___redArg(v_t_295_, v_fanout_297_);
return v___x_298_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_choice_elim___redArg(lean_object* v_t_299_, lean_object* v_choice_300_){
_start:
{
lean_object* v___x_301_; 
v___x_301_ = lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorElim___redArg(v_t_299_, v_choice_300_);
return v___x_301_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_choice_elim(lean_object* v_Op_302_, lean_object* v_V_303_, lean_object* v_Label_304_, lean_object* v_motive_305_, lean_object* v_t_306_, lean_object* v_h_307_, lean_object* v_choice_308_){
_start:
{
lean_object* v___x_309_; 
v___x_309_ = lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorElim___redArg(v_t_306_, v_choice_308_);
return v___x_309_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_op_elim___redArg(lean_object* v_t_310_, lean_object* v_op_311_){
_start:
{
lean_object* v___x_312_; 
v___x_312_ = lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorElim___redArg(v_t_310_, v_op_311_);
return v___x_312_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_op_elim(lean_object* v_Op_313_, lean_object* v_V_314_, lean_object* v_Label_315_, lean_object* v_motive_316_, lean_object* v_t_317_, lean_object* v_h_318_, lean_object* v_op_319_){
_start:
{
lean_object* v___x_320_; 
v___x_320_ = lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorElim___redArg(v_t_317_, v_op_319_);
return v___x_320_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_loop_elim___redArg(lean_object* v_t_321_, lean_object* v_loop_322_){
_start:
{
lean_object* v___x_323_; 
v___x_323_ = lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorElim___redArg(v_t_321_, v_loop_322_);
return v___x_323_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_loop_elim(lean_object* v_Op_324_, lean_object* v_V_325_, lean_object* v_Label_326_, lean_object* v_motive_327_, lean_object* v_t_328_, lean_object* v_h_329_, lean_object* v_loop_330_){
_start:
{
lean_object* v___x_331_; 
v___x_331_ = lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorElim___redArg(v_t_328_, v_loop_330_);
return v___x_331_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_apply_elim___redArg(lean_object* v_t_332_, lean_object* v_apply_333_){
_start:
{
lean_object* v___x_334_; 
v___x_334_ = lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorElim___redArg(v_t_332_, v_apply_333_);
return v___x_334_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_apply_elim(lean_object* v_Op_335_, lean_object* v_V_336_, lean_object* v_Label_337_, lean_object* v_motive_338_, lean_object* v_t_339_, lean_object* v_h_340_, lean_object* v_apply_341_){
_start:
{
lean_object* v___x_342_; 
v___x_342_ = lp_arrow__minimality__lib_StructuralInfo_TaggedFreerArrow_ctorElim___redArg(v_t_339_, v_apply_341_);
return v___x_342_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_ctorIdx(uint8_t v_x_343_){
_start:
{
switch(v_x_343_)
{
case 0:
{
lean_object* v___x_344_; 
v___x_344_ = lean_unsigned_to_nat(0u);
return v___x_344_;
}
case 1:
{
lean_object* v___x_345_; 
v___x_345_ = lean_unsigned_to_nat(1u);
return v___x_345_;
}
case 2:
{
lean_object* v___x_346_; 
v___x_346_ = lean_unsigned_to_nat(2u);
return v___x_346_;
}
case 3:
{
lean_object* v___x_347_; 
v___x_347_ = lean_unsigned_to_nat(3u);
return v___x_347_;
}
case 4:
{
lean_object* v___x_348_; 
v___x_348_ = lean_unsigned_to_nat(4u);
return v___x_348_;
}
case 5:
{
lean_object* v___x_349_; 
v___x_349_ = lean_unsigned_to_nat(5u);
return v___x_349_;
}
default: 
{
lean_object* v___x_350_; 
v___x_350_ = lean_unsigned_to_nat(6u);
return v___x_350_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_ctorIdx___boxed(lean_object* v_x_351_){
_start:
{
uint8_t v_x_boxed_352_; lean_object* v_res_353_; 
v_x_boxed_352_ = lean_unbox(v_x_351_);
v_res_353_ = lp_arrow__minimality__lib_StructuralInfo_StructFact_ctorIdx(v_x_boxed_352_);
return v_res_353_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_ctorElim___redArg(lean_object* v_k_354_){
_start:
{
lean_inc(v_k_354_);
return v_k_354_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_ctorElim___redArg___boxed(lean_object* v_k_355_){
_start:
{
lean_object* v_res_356_; 
v_res_356_ = lp_arrow__minimality__lib_StructuralInfo_StructFact_ctorElim___redArg(v_k_355_);
lean_dec(v_k_355_);
return v_res_356_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_ctorElim(lean_object* v_motive_357_, lean_object* v_ctorIdx_358_, uint8_t v_t_359_, lean_object* v_h_360_, lean_object* v_k_361_){
_start:
{
lean_inc(v_k_361_);
return v_k_361_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_ctorElim___boxed(lean_object* v_motive_362_, lean_object* v_ctorIdx_363_, lean_object* v_t_364_, lean_object* v_h_365_, lean_object* v_k_366_){
_start:
{
uint8_t v_t_boxed_367_; lean_object* v_res_368_; 
v_t_boxed_367_ = lean_unbox(v_t_364_);
v_res_368_ = lp_arrow__minimality__lib_StructuralInfo_StructFact_ctorElim(v_motive_362_, v_ctorIdx_363_, v_t_boxed_367_, v_h_365_, v_k_366_);
lean_dec(v_k_366_);
lean_dec(v_ctorIdx_363_);
return v_res_368_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_fanout__independence_elim___redArg(lean_object* v_fanout__independence_369_){
_start:
{
lean_inc(v_fanout__independence_369_);
return v_fanout__independence_369_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_fanout__independence_elim___redArg___boxed(lean_object* v_fanout__independence_370_){
_start:
{
lean_object* v_res_371_; 
v_res_371_ = lp_arrow__minimality__lib_StructuralInfo_StructFact_fanout__independence_elim___redArg(v_fanout__independence_370_);
lean_dec(v_fanout__independence_370_);
return v_res_371_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_fanout__independence_elim(lean_object* v_motive_372_, uint8_t v_t_373_, lean_object* v_h_374_, lean_object* v_fanout__independence_375_){
_start:
{
lean_inc(v_fanout__independence_375_);
return v_fanout__independence_375_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_fanout__independence_elim___boxed(lean_object* v_motive_376_, lean_object* v_t_377_, lean_object* v_h_378_, lean_object* v_fanout__independence_379_){
_start:
{
uint8_t v_t_boxed_380_; lean_object* v_res_381_; 
v_t_boxed_380_ = lean_unbox(v_t_377_);
v_res_381_ = lp_arrow__minimality__lib_StructuralInfo_StructFact_fanout__independence_elim(v_motive_376_, v_t_boxed_380_, v_h_378_, v_fanout__independence_379_);
lean_dec(v_fanout__independence_379_);
return v_res_381_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_branching_elim___redArg(lean_object* v_branching_382_){
_start:
{
lean_inc(v_branching_382_);
return v_branching_382_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_branching_elim___redArg___boxed(lean_object* v_branching_383_){
_start:
{
lean_object* v_res_384_; 
v_res_384_ = lp_arrow__minimality__lib_StructuralInfo_StructFact_branching_elim___redArg(v_branching_383_);
lean_dec(v_branching_383_);
return v_res_384_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_branching_elim(lean_object* v_motive_385_, uint8_t v_t_386_, lean_object* v_h_387_, lean_object* v_branching_388_){
_start:
{
lean_inc(v_branching_388_);
return v_branching_388_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_branching_elim___boxed(lean_object* v_motive_389_, lean_object* v_t_390_, lean_object* v_h_391_, lean_object* v_branching_392_){
_start:
{
uint8_t v_t_boxed_393_; lean_object* v_res_394_; 
v_t_boxed_393_ = lean_unbox(v_t_390_);
v_res_394_ = lp_arrow__minimality__lib_StructuralInfo_StructFact_branching_elim(v_motive_389_, v_t_boxed_393_, v_h_391_, v_branching_392_);
lean_dec(v_branching_392_);
return v_res_394_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_per__branch__effects_elim___redArg(lean_object* v_per__branch__effects_395_){
_start:
{
lean_inc(v_per__branch__effects_395_);
return v_per__branch__effects_395_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_per__branch__effects_elim___redArg___boxed(lean_object* v_per__branch__effects_396_){
_start:
{
lean_object* v_res_397_; 
v_res_397_ = lp_arrow__minimality__lib_StructuralInfo_StructFact_per__branch__effects_elim___redArg(v_per__branch__effects_396_);
lean_dec(v_per__branch__effects_396_);
return v_res_397_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_per__branch__effects_elim(lean_object* v_motive_398_, uint8_t v_t_399_, lean_object* v_h_400_, lean_object* v_per__branch__effects_401_){
_start:
{
lean_inc(v_per__branch__effects_401_);
return v_per__branch__effects_401_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_per__branch__effects_elim___boxed(lean_object* v_motive_402_, lean_object* v_t_403_, lean_object* v_h_404_, lean_object* v_per__branch__effects_405_){
_start:
{
uint8_t v_t_boxed_406_; lean_object* v_res_407_; 
v_t_boxed_406_ = lean_unbox(v_t_403_);
v_res_407_ = lp_arrow__minimality__lib_StructuralInfo_StructFact_per__branch__effects_elim(v_motive_402_, v_t_boxed_406_, v_h_404_, v_per__branch__effects_405_);
lean_dec(v_per__branch__effects_405_);
return v_res_407_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_dependency__graph_elim___redArg(lean_object* v_dependency__graph_408_){
_start:
{
lean_inc(v_dependency__graph_408_);
return v_dependency__graph_408_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_dependency__graph_elim___redArg___boxed(lean_object* v_dependency__graph_409_){
_start:
{
lean_object* v_res_410_; 
v_res_410_ = lp_arrow__minimality__lib_StructuralInfo_StructFact_dependency__graph_elim___redArg(v_dependency__graph_409_);
lean_dec(v_dependency__graph_409_);
return v_res_410_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_dependency__graph_elim(lean_object* v_motive_411_, uint8_t v_t_412_, lean_object* v_h_413_, lean_object* v_dependency__graph_414_){
_start:
{
lean_inc(v_dependency__graph_414_);
return v_dependency__graph_414_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_dependency__graph_elim___boxed(lean_object* v_motive_415_, lean_object* v_t_416_, lean_object* v_h_417_, lean_object* v_dependency__graph_418_){
_start:
{
uint8_t v_t_boxed_419_; lean_object* v_res_420_; 
v_t_boxed_419_ = lean_unbox(v_t_416_);
v_res_420_ = lp_arrow__minimality__lib_StructuralInfo_StructFact_dependency__graph_elim(v_motive_415_, v_t_boxed_419_, v_h_417_, v_dependency__graph_418_);
lean_dec(v_dependency__graph_418_);
return v_res_420_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_feedback_elim___redArg(lean_object* v_feedback_421_){
_start:
{
lean_inc(v_feedback_421_);
return v_feedback_421_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_feedback_elim___redArg___boxed(lean_object* v_feedback_422_){
_start:
{
lean_object* v_res_423_; 
v_res_423_ = lp_arrow__minimality__lib_StructuralInfo_StructFact_feedback_elim___redArg(v_feedback_422_);
lean_dec(v_feedback_422_);
return v_res_423_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_feedback_elim(lean_object* v_motive_424_, uint8_t v_t_425_, lean_object* v_h_426_, lean_object* v_feedback_427_){
_start:
{
lean_inc(v_feedback_427_);
return v_feedback_427_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_feedback_elim___boxed(lean_object* v_motive_428_, lean_object* v_t_429_, lean_object* v_h_430_, lean_object* v_feedback_431_){
_start:
{
uint8_t v_t_boxed_432_; lean_object* v_res_433_; 
v_t_boxed_432_ = lean_unbox(v_t_429_);
v_res_433_ = lp_arrow__minimality__lib_StructuralInfo_StructFact_feedback_elim(v_motive_428_, v_t_boxed_432_, v_h_430_, v_feedback_431_);
lean_dec(v_feedback_431_);
return v_res_433_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_effect__set_elim___redArg(lean_object* v_effect__set_434_){
_start:
{
lean_inc(v_effect__set_434_);
return v_effect__set_434_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_effect__set_elim___redArg___boxed(lean_object* v_effect__set_435_){
_start:
{
lean_object* v_res_436_; 
v_res_436_ = lp_arrow__minimality__lib_StructuralInfo_StructFact_effect__set_elim___redArg(v_effect__set_435_);
lean_dec(v_effect__set_435_);
return v_res_436_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_effect__set_elim(lean_object* v_motive_437_, uint8_t v_t_438_, lean_object* v_h_439_, lean_object* v_effect__set_440_){
_start:
{
lean_inc(v_effect__set_440_);
return v_effect__set_440_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_effect__set_elim___boxed(lean_object* v_motive_441_, lean_object* v_t_442_, lean_object* v_h_443_, lean_object* v_effect__set_444_){
_start:
{
uint8_t v_t_boxed_445_; lean_object* v_res_446_; 
v_t_boxed_445_ = lean_unbox(v_t_442_);
v_res_446_ = lp_arrow__minimality__lib_StructuralInfo_StructFact_effect__set_elim(v_motive_441_, v_t_boxed_445_, v_h_443_, v_effect__set_444_);
lean_dec(v_effect__set_444_);
return v_res_446_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_op__provenance_elim___redArg(lean_object* v_op__provenance_447_){
_start:
{
lean_inc(v_op__provenance_447_);
return v_op__provenance_447_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_op__provenance_elim___redArg___boxed(lean_object* v_op__provenance_448_){
_start:
{
lean_object* v_res_449_; 
v_res_449_ = lp_arrow__minimality__lib_StructuralInfo_StructFact_op__provenance_elim___redArg(v_op__provenance_448_);
lean_dec(v_op__provenance_448_);
return v_res_449_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_op__provenance_elim(lean_object* v_motive_450_, uint8_t v_t_451_, lean_object* v_h_452_, lean_object* v_op__provenance_453_){
_start:
{
lean_inc(v_op__provenance_453_);
return v_op__provenance_453_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_StructFact_op__provenance_elim___boxed(lean_object* v_motive_454_, lean_object* v_t_455_, lean_object* v_h_456_, lean_object* v_op__provenance_457_){
_start:
{
uint8_t v_t_boxed_458_; lean_object* v_res_459_; 
v_t_boxed_458_ = lean_unbox(v_t_455_);
v_res_459_ = lp_arrow__minimality__lib_StructuralInfo_StructFact_op__provenance_elim(v_motive_454_, v_t_boxed_458_, v_h_456_, v_op__provenance_457_);
lean_dec(v_op__provenance_457_);
return v_res_459_;
}
}
static lean_object* _init_lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__6(void){
_start:
{
lean_object* v___x_497_; lean_object* v___x_498_; 
v___x_497_ = ((lean_object*)(lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__5));
v___x_498_ = l_String_toRawSubstring_x27(v___x_497_);
return v___x_498_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1(lean_object* v_x_517_, lean_object* v_a_518_, lean_object* v_a_519_){
_start:
{
lean_object* v___x_520_; uint8_t v___x_521_; 
v___x_520_ = ((lean_object*)(lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__2));
lean_inc(v_x_517_);
v___x_521_ = l_Lean_Syntax_isOfKind(v_x_517_, v___x_520_);
if (v___x_521_ == 0)
{
lean_object* v___x_522_; lean_object* v___x_523_; 
lean_dec(v_x_517_);
v___x_522_ = lean_box(1);
v___x_523_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_523_, 0, v___x_522_);
lean_ctor_set(v___x_523_, 1, v_a_519_);
return v___x_523_;
}
else
{
lean_object* v_quotContext_524_; lean_object* v_currMacroScope_525_; lean_object* v_ref_526_; lean_object* v___x_527_; lean_object* v___x_528_; lean_object* v___x_529_; lean_object* v___x_530_; uint8_t v___x_531_; lean_object* v___x_532_; lean_object* v___x_533_; lean_object* v___x_534_; lean_object* v___x_535_; lean_object* v___x_536_; lean_object* v___x_537_; lean_object* v___x_538_; lean_object* v___x_539_; lean_object* v___x_540_; lean_object* v___x_541_; lean_object* v___x_542_; 
v_quotContext_524_ = lean_ctor_get(v_a_518_, 1);
v_currMacroScope_525_ = lean_ctor_get(v_a_518_, 2);
v_ref_526_ = lean_ctor_get(v_a_518_, 5);
v___x_527_ = lean_unsigned_to_nat(0u);
v___x_528_ = l_Lean_Syntax_getArg(v_x_517_, v___x_527_);
v___x_529_ = lean_unsigned_to_nat(2u);
v___x_530_ = l_Lean_Syntax_getArg(v_x_517_, v___x_529_);
lean_dec(v_x_517_);
v___x_531_ = 0;
v___x_532_ = l_Lean_SourceInfo_fromRef(v_ref_526_, v___x_531_);
v___x_533_ = ((lean_object*)(lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__4));
v___x_534_ = lean_obj_once(&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__6, &lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__6_once, _init_lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__6);
v___x_535_ = ((lean_object*)(lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__9));
lean_inc(v_currMacroScope_525_);
lean_inc(v_quotContext_524_);
v___x_536_ = l_Lean_addMacroScope(v_quotContext_524_, v___x_535_, v_currMacroScope_525_);
v___x_537_ = ((lean_object*)(lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__12));
lean_inc_n(v___x_532_, 2);
v___x_538_ = lean_alloc_ctor(3, 4, 0);
lean_ctor_set(v___x_538_, 0, v___x_532_);
lean_ctor_set(v___x_538_, 1, v___x_534_);
lean_ctor_set(v___x_538_, 2, v___x_536_);
lean_ctor_set(v___x_538_, 3, v___x_537_);
v___x_539_ = ((lean_object*)(lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__14));
v___x_540_ = l_Lean_Syntax_node2(v___x_532_, v___x_539_, v___x_528_, v___x_530_);
v___x_541_ = l_Lean_Syntax_node2(v___x_532_, v___x_533_, v___x_538_, v___x_540_);
v___x_542_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_542_, 0, v___x_541_);
lean_ctor_set(v___x_542_, 1, v_a_519_);
return v___x_542_;
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___boxed(lean_object* v_x_543_, lean_object* v_a_544_, lean_object* v_a_545_){
_start:
{
lean_object* v_res_546_; 
v_res_546_ = lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1(v_x_543_, v_a_544_, v_a_545_);
lean_dec_ref(v_a_544_);
return v_res_546_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______unexpand__StructuralInfo__FactSet__subset__1(lean_object* v_x_550_, lean_object* v_a_551_, lean_object* v_a_552_){
_start:
{
lean_object* v___x_553_; uint8_t v___x_554_; 
v___x_553_ = ((lean_object*)(lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__4));
lean_inc(v_x_550_);
v___x_554_ = l_Lean_Syntax_isOfKind(v_x_550_, v___x_553_);
if (v___x_554_ == 0)
{
lean_object* v___x_555_; lean_object* v___x_556_; 
lean_dec(v_x_550_);
v___x_555_ = lean_box(0);
v___x_556_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_556_, 0, v___x_555_);
lean_ctor_set(v___x_556_, 1, v_a_552_);
return v___x_556_;
}
else
{
lean_object* v___x_557_; lean_object* v___x_558_; lean_object* v___x_559_; uint8_t v___x_560_; 
v___x_557_ = lean_unsigned_to_nat(0u);
v___x_558_ = l_Lean_Syntax_getArg(v_x_550_, v___x_557_);
v___x_559_ = ((lean_object*)(lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______unexpand__StructuralInfo__FactSet__subset__1___closed__1));
lean_inc(v___x_558_);
v___x_560_ = l_Lean_Syntax_isOfKind(v___x_558_, v___x_559_);
if (v___x_560_ == 0)
{
lean_object* v___x_561_; lean_object* v___x_562_; 
lean_dec(v___x_558_);
lean_dec(v_x_550_);
v___x_561_ = lean_box(0);
v___x_562_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_562_, 0, v___x_561_);
lean_ctor_set(v___x_562_, 1, v_a_552_);
return v___x_562_;
}
else
{
lean_object* v___x_563_; lean_object* v___x_564_; lean_object* v___x_565_; uint8_t v___x_566_; 
v___x_563_ = lean_unsigned_to_nat(1u);
v___x_564_ = l_Lean_Syntax_getArg(v_x_550_, v___x_563_);
lean_dec(v_x_550_);
v___x_565_ = lean_unsigned_to_nat(2u);
lean_inc(v___x_564_);
v___x_566_ = l_Lean_Syntax_matchesNull(v___x_564_, v___x_565_);
if (v___x_566_ == 0)
{
lean_object* v___x_567_; lean_object* v___x_568_; 
lean_dec(v___x_564_);
lean_dec(v___x_558_);
v___x_567_ = lean_box(0);
v___x_568_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_568_, 0, v___x_567_);
lean_ctor_set(v___x_568_, 1, v_a_552_);
return v___x_568_;
}
else
{
lean_object* v___x_569_; lean_object* v___x_570_; lean_object* v_ref_571_; uint8_t v___x_572_; lean_object* v___x_573_; lean_object* v___x_574_; lean_object* v___x_575_; lean_object* v___x_576_; lean_object* v___x_577_; lean_object* v___x_578_; 
v___x_569_ = l_Lean_Syntax_getArg(v___x_564_, v___x_557_);
v___x_570_ = l_Lean_Syntax_getArg(v___x_564_, v___x_563_);
lean_dec(v___x_564_);
v_ref_571_ = l_Lean_replaceRef(v___x_558_, v_a_551_);
lean_dec(v___x_558_);
v___x_572_ = 0;
v___x_573_ = l_Lean_SourceInfo_fromRef(v_ref_571_, v___x_572_);
lean_dec(v_ref_571_);
v___x_574_ = ((lean_object*)(lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__2));
v___x_575_ = ((lean_object*)(lp_arrow__minimality__lib_StructuralInfo_term___u2286f___00__closed__5));
lean_inc(v___x_573_);
v___x_576_ = lean_alloc_ctor(2, 2, 0);
lean_ctor_set(v___x_576_, 0, v___x_573_);
lean_ctor_set(v___x_576_, 1, v___x_575_);
v___x_577_ = l_Lean_Syntax_node3(v___x_573_, v___x_574_, v___x_569_, v___x_576_, v___x_570_);
v___x_578_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_578_, 0, v___x_577_);
lean_ctor_set(v___x_578_, 1, v_a_552_);
return v___x_578_;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______unexpand__StructuralInfo__FactSet__subset__1___boxed(lean_object* v_x_579_, lean_object* v_a_580_, lean_object* v_a_581_){
_start:
{
lean_object* v_res_582_; 
v_res_582_ = lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______unexpand__StructuralInfo__FactSet__subset__1(v_x_579_, v_a_580_, v_a_581_);
lean_dec(v_a_580_);
return v_res_582_;
}
}
static lean_object* _init_lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__1(void){
_start:
{
lean_object* v___x_604_; lean_object* v___x_605_; 
v___x_604_ = ((lean_object*)(lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__0));
v___x_605_ = l_String_toRawSubstring_x27(v___x_604_);
return v___x_605_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1(lean_object* v_x_620_, lean_object* v_a_621_, lean_object* v_a_622_){
_start:
{
lean_object* v___x_623_; uint8_t v___x_624_; 
v___x_623_ = ((lean_object*)(lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__1));
lean_inc(v_x_620_);
v___x_624_ = l_Lean_Syntax_isOfKind(v_x_620_, v___x_623_);
if (v___x_624_ == 0)
{
lean_object* v___x_625_; lean_object* v___x_626_; 
lean_dec(v_x_620_);
v___x_625_ = lean_box(1);
v___x_626_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_626_, 0, v___x_625_);
lean_ctor_set(v___x_626_, 1, v_a_622_);
return v___x_626_;
}
else
{
lean_object* v_quotContext_627_; lean_object* v_currMacroScope_628_; lean_object* v_ref_629_; lean_object* v___x_630_; lean_object* v___x_631_; lean_object* v___x_632_; lean_object* v___x_633_; uint8_t v___x_634_; lean_object* v___x_635_; lean_object* v___x_636_; lean_object* v___x_637_; lean_object* v___x_638_; lean_object* v___x_639_; lean_object* v___x_640_; lean_object* v___x_641_; lean_object* v___x_642_; lean_object* v___x_643_; lean_object* v___x_644_; lean_object* v___x_645_; 
v_quotContext_627_ = lean_ctor_get(v_a_621_, 1);
v_currMacroScope_628_ = lean_ctor_get(v_a_621_, 2);
v_ref_629_ = lean_ctor_get(v_a_621_, 5);
v___x_630_ = lean_unsigned_to_nat(0u);
v___x_631_ = l_Lean_Syntax_getArg(v_x_620_, v___x_630_);
v___x_632_ = lean_unsigned_to_nat(2u);
v___x_633_ = l_Lean_Syntax_getArg(v_x_620_, v___x_632_);
lean_dec(v_x_620_);
v___x_634_ = 0;
v___x_635_ = l_Lean_SourceInfo_fromRef(v_ref_629_, v___x_634_);
v___x_636_ = ((lean_object*)(lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__4));
v___x_637_ = lean_obj_once(&lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__1, &lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__1_once, _init_lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__1);
v___x_638_ = ((lean_object*)(lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__3));
lean_inc(v_currMacroScope_628_);
lean_inc(v_quotContext_627_);
v___x_639_ = l_Lean_addMacroScope(v_quotContext_627_, v___x_638_, v_currMacroScope_628_);
v___x_640_ = ((lean_object*)(lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___closed__6));
lean_inc_n(v___x_635_, 2);
v___x_641_ = lean_alloc_ctor(3, 4, 0);
lean_ctor_set(v___x_641_, 0, v___x_635_);
lean_ctor_set(v___x_641_, 1, v___x_637_);
lean_ctor_set(v___x_641_, 2, v___x_639_);
lean_ctor_set(v___x_641_, 3, v___x_640_);
v___x_642_ = ((lean_object*)(lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__14));
v___x_643_ = l_Lean_Syntax_node2(v___x_635_, v___x_642_, v___x_631_, v___x_633_);
v___x_644_ = l_Lean_Syntax_node2(v___x_635_, v___x_636_, v___x_641_, v___x_643_);
v___x_645_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_645_, 0, v___x_644_);
lean_ctor_set(v___x_645_, 1, v_a_622_);
return v___x_645_;
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1___boxed(lean_object* v_x_646_, lean_object* v_a_647_, lean_object* v_a_648_){
_start:
{
lean_object* v_res_649_; 
v_res_649_ = lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2208f____1(v_x_646_, v_a_647_, v_a_648_);
lean_dec_ref(v_a_647_);
return v_res_649_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______unexpand__StructuralInfo__FactSet__mem__1(lean_object* v_x_650_, lean_object* v_a_651_, lean_object* v_a_652_){
_start:
{
lean_object* v___x_653_; uint8_t v___x_654_; 
v___x_653_ = ((lean_object*)(lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______macroRules__StructuralInfo__term___u2286f____1___closed__4));
lean_inc(v_x_650_);
v___x_654_ = l_Lean_Syntax_isOfKind(v_x_650_, v___x_653_);
if (v___x_654_ == 0)
{
lean_object* v___x_655_; lean_object* v___x_656_; 
lean_dec(v_x_650_);
v___x_655_ = lean_box(0);
v___x_656_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_656_, 0, v___x_655_);
lean_ctor_set(v___x_656_, 1, v_a_652_);
return v___x_656_;
}
else
{
lean_object* v___x_657_; lean_object* v___x_658_; lean_object* v___x_659_; uint8_t v___x_660_; 
v___x_657_ = lean_unsigned_to_nat(0u);
v___x_658_ = l_Lean_Syntax_getArg(v_x_650_, v___x_657_);
v___x_659_ = ((lean_object*)(lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______unexpand__StructuralInfo__FactSet__subset__1___closed__1));
lean_inc(v___x_658_);
v___x_660_ = l_Lean_Syntax_isOfKind(v___x_658_, v___x_659_);
if (v___x_660_ == 0)
{
lean_object* v___x_661_; lean_object* v___x_662_; 
lean_dec(v___x_658_);
lean_dec(v_x_650_);
v___x_661_ = lean_box(0);
v___x_662_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_662_, 0, v___x_661_);
lean_ctor_set(v___x_662_, 1, v_a_652_);
return v___x_662_;
}
else
{
lean_object* v___x_663_; lean_object* v___x_664_; lean_object* v___x_665_; uint8_t v___x_666_; 
v___x_663_ = lean_unsigned_to_nat(1u);
v___x_664_ = l_Lean_Syntax_getArg(v_x_650_, v___x_663_);
lean_dec(v_x_650_);
v___x_665_ = lean_unsigned_to_nat(2u);
lean_inc(v___x_664_);
v___x_666_ = l_Lean_Syntax_matchesNull(v___x_664_, v___x_665_);
if (v___x_666_ == 0)
{
lean_object* v___x_667_; lean_object* v___x_668_; 
lean_dec(v___x_664_);
lean_dec(v___x_658_);
v___x_667_ = lean_box(0);
v___x_668_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_668_, 0, v___x_667_);
lean_ctor_set(v___x_668_, 1, v_a_652_);
return v___x_668_;
}
else
{
lean_object* v___x_669_; lean_object* v___x_670_; lean_object* v_ref_671_; uint8_t v___x_672_; lean_object* v___x_673_; lean_object* v___x_674_; lean_object* v___x_675_; lean_object* v___x_676_; lean_object* v___x_677_; lean_object* v___x_678_; 
v___x_669_ = l_Lean_Syntax_getArg(v___x_664_, v___x_657_);
v___x_670_ = l_Lean_Syntax_getArg(v___x_664_, v___x_663_);
lean_dec(v___x_664_);
v_ref_671_ = l_Lean_replaceRef(v___x_658_, v_a_651_);
lean_dec(v___x_658_);
v___x_672_ = 0;
v___x_673_ = l_Lean_SourceInfo_fromRef(v_ref_671_, v___x_672_);
lean_dec(v_ref_671_);
v___x_674_ = ((lean_object*)(lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__1));
v___x_675_ = ((lean_object*)(lp_arrow__minimality__lib_StructuralInfo_term___u2208f___00__closed__2));
lean_inc(v___x_673_);
v___x_676_ = lean_alloc_ctor(2, 2, 0);
lean_ctor_set(v___x_676_, 0, v___x_673_);
lean_ctor_set(v___x_676_, 1, v___x_675_);
v___x_677_ = l_Lean_Syntax_node3(v___x_673_, v___x_674_, v___x_669_, v___x_676_, v___x_670_);
v___x_678_ = lean_alloc_ctor(0, 2, 0);
lean_ctor_set(v___x_678_, 0, v___x_677_);
lean_ctor_set(v___x_678_, 1, v_a_652_);
return v___x_678_;
}
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______unexpand__StructuralInfo__FactSet__mem__1___boxed(lean_object* v_x_679_, lean_object* v_a_680_, lean_object* v_a_681_){
_start:
{
lean_object* v_res_682_; 
v_res_682_ = lp_arrow__minimality__lib_StructuralInfo___aux__ArrowMinimalityLib__StructuralInfo______unexpand__StructuralInfo__FactSet__mem__1(v_x_679_, v_a_680_, v_a_681_);
lean_dec(v_a_680_);
return v_res_682_;
}
}
LEAN_EXPORT uint8_t lp_arrow__minimality__lib_StructuralInfo_containsFanout___redArg(lean_object* v_x_683_){
_start:
{
lean_object* v_f_685_; lean_object* v_g_686_; 
switch(lean_obj_tag(v_x_683_))
{
case 0:
{
uint8_t v___x_689_; 
v___x_689_ = 0;
return v___x_689_;
}
case 2:
{
uint8_t v___x_690_; 
v___x_690_ = 1;
return v___x_690_;
}
case 4:
{
lean_object* v_k_691_; 
v_k_691_ = lean_ctor_get(v_x_683_, 1);
v_x_683_ = v_k_691_;
goto _start;
}
case 5:
{
lean_object* v_f_693_; 
v_f_693_ = lean_ctor_get(v_x_683_, 0);
v_x_683_ = v_f_693_;
goto _start;
}
case 6:
{
lean_object* v_f_695_; 
v_f_695_ = lean_ctor_get(v_x_683_, 0);
v_x_683_ = v_f_695_;
goto _start;
}
default: 
{
lean_object* v_f_697_; lean_object* v_g_698_; 
v_f_697_ = lean_ctor_get(v_x_683_, 0);
v_g_698_ = lean_ctor_get(v_x_683_, 1);
v_f_685_ = v_f_697_;
v_g_686_ = v_g_698_;
goto v___jp_684_;
}
}
v___jp_684_:
{
uint8_t v___x_687_; 
v___x_687_ = lp_arrow__minimality__lib_StructuralInfo_containsFanout___redArg(v_f_685_);
if (v___x_687_ == 0)
{
v_x_683_ = v_g_686_;
goto _start;
}
else
{
return v___x_687_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_containsFanout___redArg___boxed(lean_object* v_x_699_){
_start:
{
uint8_t v_res_700_; lean_object* v_r_701_; 
v_res_700_ = lp_arrow__minimality__lib_StructuralInfo_containsFanout___redArg(v_x_699_);
lean_dec_ref(v_x_699_);
v_r_701_ = lean_box(v_res_700_);
return v_r_701_;
}
}
LEAN_EXPORT uint8_t lp_arrow__minimality__lib_StructuralInfo_containsFanout(lean_object* v_Op_702_, lean_object* v_V_703_, lean_object* v_x_704_){
_start:
{
uint8_t v___x_705_; 
v___x_705_ = lp_arrow__minimality__lib_StructuralInfo_containsFanout___redArg(v_x_704_);
return v___x_705_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_containsFanout___boxed(lean_object* v_Op_706_, lean_object* v_V_707_, lean_object* v_x_708_){
_start:
{
uint8_t v_res_709_; lean_object* v_r_710_; 
v_res_709_ = lp_arrow__minimality__lib_StructuralInfo_containsFanout(v_Op_706_, v_V_707_, v_x_708_);
lean_dec_ref(v_x_708_);
v_r_710_ = lean_box(v_res_709_);
return v_r_710_;
}
}
LEAN_EXPORT uint8_t lp_arrow__minimality__lib_StructuralInfo_containsChoice___redArg(lean_object* v_x_711_){
_start:
{
lean_object* v_f_713_; lean_object* v_g_714_; 
switch(lean_obj_tag(v_x_711_))
{
case 0:
{
uint8_t v___x_717_; 
v___x_717_ = 0;
return v___x_717_;
}
case 3:
{
uint8_t v___x_718_; 
v___x_718_ = 1;
return v___x_718_;
}
case 4:
{
lean_object* v_k_719_; 
v_k_719_ = lean_ctor_get(v_x_711_, 1);
v_x_711_ = v_k_719_;
goto _start;
}
case 5:
{
lean_object* v_f_721_; 
v_f_721_ = lean_ctor_get(v_x_711_, 0);
v_x_711_ = v_f_721_;
goto _start;
}
case 6:
{
lean_object* v_f_723_; 
v_f_723_ = lean_ctor_get(v_x_711_, 0);
v_x_711_ = v_f_723_;
goto _start;
}
default: 
{
lean_object* v_f_725_; lean_object* v_g_726_; 
v_f_725_ = lean_ctor_get(v_x_711_, 0);
v_g_726_ = lean_ctor_get(v_x_711_, 1);
v_f_713_ = v_f_725_;
v_g_714_ = v_g_726_;
goto v___jp_712_;
}
}
v___jp_712_:
{
uint8_t v___x_715_; 
v___x_715_ = lp_arrow__minimality__lib_StructuralInfo_containsChoice___redArg(v_f_713_);
if (v___x_715_ == 0)
{
v_x_711_ = v_g_714_;
goto _start;
}
else
{
return v___x_715_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_containsChoice___redArg___boxed(lean_object* v_x_727_){
_start:
{
uint8_t v_res_728_; lean_object* v_r_729_; 
v_res_728_ = lp_arrow__minimality__lib_StructuralInfo_containsChoice___redArg(v_x_727_);
lean_dec_ref(v_x_727_);
v_r_729_ = lean_box(v_res_728_);
return v_r_729_;
}
}
LEAN_EXPORT uint8_t lp_arrow__minimality__lib_StructuralInfo_containsChoice(lean_object* v_Op_730_, lean_object* v_V_731_, lean_object* v_x_732_){
_start:
{
uint8_t v___x_733_; 
v___x_733_ = lp_arrow__minimality__lib_StructuralInfo_containsChoice___redArg(v_x_732_);
return v___x_733_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_containsChoice___boxed(lean_object* v_Op_734_, lean_object* v_V_735_, lean_object* v_x_736_){
_start:
{
uint8_t v_res_737_; lean_object* v_r_738_; 
v_res_737_ = lp_arrow__minimality__lib_StructuralInfo_containsChoice(v_Op_734_, v_V_735_, v_x_736_);
lean_dec_ref(v_x_736_);
v_r_738_ = lean_box(v_res_737_);
return v_r_738_;
}
}
LEAN_EXPORT uint8_t lp_arrow__minimality__lib_StructuralInfo_containsLoop___redArg(lean_object* v_x_739_){
_start:
{
lean_object* v_f_741_; lean_object* v_g_742_; 
switch(lean_obj_tag(v_x_739_))
{
case 0:
{
uint8_t v___x_745_; 
v___x_745_ = 0;
return v___x_745_;
}
case 4:
{
lean_object* v_k_746_; 
v_k_746_ = lean_ctor_get(v_x_739_, 1);
v_x_739_ = v_k_746_;
goto _start;
}
case 5:
{
uint8_t v___x_748_; 
v___x_748_ = 1;
return v___x_748_;
}
case 6:
{
lean_object* v_f_749_; 
v_f_749_ = lean_ctor_get(v_x_739_, 0);
v_x_739_ = v_f_749_;
goto _start;
}
default: 
{
lean_object* v_f_751_; lean_object* v_g_752_; 
v_f_751_ = lean_ctor_get(v_x_739_, 0);
v_g_752_ = lean_ctor_get(v_x_739_, 1);
v_f_741_ = v_f_751_;
v_g_742_ = v_g_752_;
goto v___jp_740_;
}
}
v___jp_740_:
{
uint8_t v___x_743_; 
v___x_743_ = lp_arrow__minimality__lib_StructuralInfo_containsLoop___redArg(v_f_741_);
if (v___x_743_ == 0)
{
v_x_739_ = v_g_742_;
goto _start;
}
else
{
return v___x_743_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_containsLoop___redArg___boxed(lean_object* v_x_753_){
_start:
{
uint8_t v_res_754_; lean_object* v_r_755_; 
v_res_754_ = lp_arrow__minimality__lib_StructuralInfo_containsLoop___redArg(v_x_753_);
lean_dec_ref(v_x_753_);
v_r_755_ = lean_box(v_res_754_);
return v_r_755_;
}
}
LEAN_EXPORT uint8_t lp_arrow__minimality__lib_StructuralInfo_containsLoop(lean_object* v_Op_756_, lean_object* v_V_757_, lean_object* v_x_758_){
_start:
{
uint8_t v___x_759_; 
v___x_759_ = lp_arrow__minimality__lib_StructuralInfo_containsLoop___redArg(v_x_758_);
return v___x_759_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_containsLoop___boxed(lean_object* v_Op_760_, lean_object* v_V_761_, lean_object* v_x_762_){
_start:
{
uint8_t v_res_763_; lean_object* v_r_764_; 
v_res_763_ = lp_arrow__minimality__lib_StructuralInfo_containsLoop(v_Op_760_, v_V_761_, v_x_762_);
lean_dec_ref(v_x_762_);
v_r_764_ = lean_box(v_res_763_);
return v_r_764_;
}
}
LEAN_EXPORT uint8_t lp_arrow__minimality__lib_StructuralInfo_containsOp___redArg(lean_object* v_x_765_){
_start:
{
lean_object* v_f_767_; lean_object* v_g_768_; 
switch(lean_obj_tag(v_x_765_))
{
case 0:
{
uint8_t v___x_771_; 
v___x_771_ = 0;
return v___x_771_;
}
case 4:
{
uint8_t v___x_772_; 
v___x_772_ = 1;
return v___x_772_;
}
case 5:
{
lean_object* v_f_773_; 
v_f_773_ = lean_ctor_get(v_x_765_, 0);
v_x_765_ = v_f_773_;
goto _start;
}
case 6:
{
lean_object* v_f_775_; 
v_f_775_ = lean_ctor_get(v_x_765_, 0);
v_x_765_ = v_f_775_;
goto _start;
}
default: 
{
lean_object* v_f_777_; lean_object* v_g_778_; 
v_f_777_ = lean_ctor_get(v_x_765_, 0);
v_g_778_ = lean_ctor_get(v_x_765_, 1);
v_f_767_ = v_f_777_;
v_g_768_ = v_g_778_;
goto v___jp_766_;
}
}
v___jp_766_:
{
uint8_t v___x_769_; 
v___x_769_ = lp_arrow__minimality__lib_StructuralInfo_containsOp___redArg(v_f_767_);
if (v___x_769_ == 0)
{
v_x_765_ = v_g_768_;
goto _start;
}
else
{
return v___x_769_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_containsOp___redArg___boxed(lean_object* v_x_779_){
_start:
{
uint8_t v_res_780_; lean_object* v_r_781_; 
v_res_780_ = lp_arrow__minimality__lib_StructuralInfo_containsOp___redArg(v_x_779_);
lean_dec_ref(v_x_779_);
v_r_781_ = lean_box(v_res_780_);
return v_r_781_;
}
}
LEAN_EXPORT uint8_t lp_arrow__minimality__lib_StructuralInfo_containsOp(lean_object* v_Op_782_, lean_object* v_V_783_, lean_object* v_x_784_){
_start:
{
uint8_t v___x_785_; 
v___x_785_ = lp_arrow__minimality__lib_StructuralInfo_containsOp___redArg(v_x_784_);
return v___x_785_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_containsOp___boxed(lean_object* v_Op_786_, lean_object* v_V_787_, lean_object* v_x_788_){
_start:
{
uint8_t v_res_789_; lean_object* v_r_790_; 
v_res_789_ = lp_arrow__minimality__lib_StructuralInfo_containsOp(v_Op_786_, v_V_787_, v_x_788_);
lean_dec_ref(v_x_788_);
v_r_790_ = lean_box(v_res_789_);
return v_r_790_;
}
}
LEAN_EXPORT uint8_t lp_arrow__minimality__lib_StructuralInfo_holds___redArg(uint8_t v_x_791_, lean_object* v_x_792_){
_start:
{
switch(v_x_791_)
{
case 0:
{
uint8_t v___x_793_; 
v___x_793_ = lp_arrow__minimality__lib_StructuralInfo_containsFanout___redArg(v_x_792_);
return v___x_793_;
}
case 3:
{
uint8_t v___x_794_; 
v___x_794_ = 1;
return v___x_794_;
}
case 4:
{
uint8_t v___x_795_; 
v___x_795_ = lp_arrow__minimality__lib_StructuralInfo_containsLoop___redArg(v_x_792_);
return v___x_795_;
}
case 5:
{
uint8_t v___x_796_; 
v___x_796_ = lp_arrow__minimality__lib_StructuralInfo_containsOp___redArg(v_x_792_);
return v___x_796_;
}
case 6:
{
uint8_t v___x_797_; 
v___x_797_ = 0;
return v___x_797_;
}
default: 
{
uint8_t v___x_798_; 
v___x_798_ = lp_arrow__minimality__lib_StructuralInfo_containsChoice___redArg(v_x_792_);
return v___x_798_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_holds___redArg___boxed(lean_object* v_x_799_, lean_object* v_x_800_){
_start:
{
uint8_t v_x_43__boxed_801_; uint8_t v_res_802_; lean_object* v_r_803_; 
v_x_43__boxed_801_ = lean_unbox(v_x_799_);
v_res_802_ = lp_arrow__minimality__lib_StructuralInfo_holds___redArg(v_x_43__boxed_801_, v_x_800_);
lean_dec_ref(v_x_800_);
v_r_803_ = lean_box(v_res_802_);
return v_r_803_;
}
}
LEAN_EXPORT uint8_t lp_arrow__minimality__lib_StructuralInfo_holds(lean_object* v_Op_804_, lean_object* v_V_805_, uint8_t v_x_806_, lean_object* v_x_807_){
_start:
{
uint8_t v___x_808_; 
v___x_808_ = lp_arrow__minimality__lib_StructuralInfo_holds___redArg(v_x_806_, v_x_807_);
return v___x_808_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_holds___boxed(lean_object* v_Op_809_, lean_object* v_V_810_, lean_object* v_x_811_, lean_object* v_x_812_){
_start:
{
uint8_t v_x_61__boxed_813_; uint8_t v_res_814_; lean_object* v_r_815_; 
v_x_61__boxed_813_ = lean_unbox(v_x_811_);
v_res_814_ = lp_arrow__minimality__lib_StructuralInfo_holds(v_Op_809_, v_V_810_, v_x_61__boxed_813_, v_x_812_);
lean_dec_ref(v_x_812_);
v_r_815_ = lean_box(v_res_814_);
return v_r_815_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_encodeMonad___redArg(lean_object* v_x_816_){
_start:
{
lean_object* v_f_818_; lean_object* v_g_819_; 
switch(lean_obj_tag(v_x_816_))
{
case 0:
{
lean_object* v_v_823_; lean_object* v___x_825_; uint8_t v_isShared_826_; uint8_t v_isSharedCheck_830_; 
v_v_823_ = lean_ctor_get(v_x_816_, 0);
v_isSharedCheck_830_ = !lean_is_exclusive(v_x_816_);
if (v_isSharedCheck_830_ == 0)
{
v___x_825_ = v_x_816_;
v_isShared_826_ = v_isSharedCheck_830_;
goto v_resetjp_824_;
}
else
{
lean_inc(v_v_823_);
lean_dec(v_x_816_);
v___x_825_ = lean_box(0);
v_isShared_826_ = v_isSharedCheck_830_;
goto v_resetjp_824_;
}
v_resetjp_824_:
{
lean_object* v___x_828_; 
if (v_isShared_826_ == 0)
{
v___x_828_ = v___x_825_;
goto v_reusejp_827_;
}
else
{
lean_object* v_reuseFailAlloc_829_; 
v_reuseFailAlloc_829_ = lean_alloc_ctor(0, 1, 0);
lean_ctor_set(v_reuseFailAlloc_829_, 0, v_v_823_);
v___x_828_ = v_reuseFailAlloc_829_;
goto v_reusejp_827_;
}
v_reusejp_827_:
{
return v___x_828_;
}
}
}
case 3:
{
lean_object* v_f_831_; lean_object* v_g_832_; lean_object* v___x_834_; uint8_t v_isShared_835_; uint8_t v_isSharedCheck_841_; 
v_f_831_ = lean_ctor_get(v_x_816_, 0);
v_g_832_ = lean_ctor_get(v_x_816_, 1);
v_isSharedCheck_841_ = !lean_is_exclusive(v_x_816_);
if (v_isSharedCheck_841_ == 0)
{
v___x_834_ = v_x_816_;
v_isShared_835_ = v_isSharedCheck_841_;
goto v_resetjp_833_;
}
else
{
lean_inc(v_g_832_);
lean_inc(v_f_831_);
lean_dec(v_x_816_);
v___x_834_ = lean_box(0);
v_isShared_835_ = v_isSharedCheck_841_;
goto v_resetjp_833_;
}
v_resetjp_833_:
{
lean_object* v___x_836_; lean_object* v___x_837_; lean_object* v___x_839_; 
v___x_836_ = lp_arrow__minimality__lib_StructuralInfo_encodeMonad___redArg(v_f_831_);
v___x_837_ = lp_arrow__minimality__lib_StructuralInfo_encodeMonad___redArg(v_g_832_);
if (v_isShared_835_ == 0)
{
lean_ctor_set_tag(v___x_834_, 2);
lean_ctor_set(v___x_834_, 1, v___x_837_);
lean_ctor_set(v___x_834_, 0, v___x_836_);
v___x_839_ = v___x_834_;
goto v_reusejp_838_;
}
else
{
lean_object* v_reuseFailAlloc_840_; 
v_reuseFailAlloc_840_ = lean_alloc_ctor(2, 2, 0);
lean_ctor_set(v_reuseFailAlloc_840_, 0, v___x_836_);
lean_ctor_set(v_reuseFailAlloc_840_, 1, v___x_837_);
v___x_839_ = v_reuseFailAlloc_840_;
goto v_reusejp_838_;
}
v_reusejp_838_:
{
return v___x_839_;
}
}
}
case 4:
{
lean_object* v_o_842_; lean_object* v_k_843_; lean_object* v___x_845_; uint8_t v_isShared_846_; uint8_t v_isSharedCheck_851_; 
v_o_842_ = lean_ctor_get(v_x_816_, 0);
v_k_843_ = lean_ctor_get(v_x_816_, 1);
v_isSharedCheck_851_ = !lean_is_exclusive(v_x_816_);
if (v_isSharedCheck_851_ == 0)
{
v___x_845_ = v_x_816_;
v_isShared_846_ = v_isSharedCheck_851_;
goto v_resetjp_844_;
}
else
{
lean_inc(v_k_843_);
lean_inc(v_o_842_);
lean_dec(v_x_816_);
v___x_845_ = lean_box(0);
v_isShared_846_ = v_isSharedCheck_851_;
goto v_resetjp_844_;
}
v_resetjp_844_:
{
lean_object* v___x_847_; lean_object* v___x_849_; 
v___x_847_ = lp_arrow__minimality__lib_StructuralInfo_encodeMonad___redArg(v_k_843_);
if (v_isShared_846_ == 0)
{
lean_ctor_set_tag(v___x_845_, 3);
lean_ctor_set(v___x_845_, 1, v___x_847_);
v___x_849_ = v___x_845_;
goto v_reusejp_848_;
}
else
{
lean_object* v_reuseFailAlloc_850_; 
v_reuseFailAlloc_850_ = lean_alloc_ctor(3, 2, 0);
lean_ctor_set(v_reuseFailAlloc_850_, 0, v_o_842_);
lean_ctor_set(v_reuseFailAlloc_850_, 1, v___x_847_);
v___x_849_ = v_reuseFailAlloc_850_;
goto v_reusejp_848_;
}
v_reusejp_848_:
{
return v___x_849_;
}
}
}
case 5:
{
lean_object* v_f_852_; lean_object* v___x_854_; uint8_t v_isShared_855_; uint8_t v_isSharedCheck_860_; 
v_f_852_ = lean_ctor_get(v_x_816_, 0);
v_isSharedCheck_860_ = !lean_is_exclusive(v_x_816_);
if (v_isSharedCheck_860_ == 0)
{
v___x_854_ = v_x_816_;
v_isShared_855_ = v_isSharedCheck_860_;
goto v_resetjp_853_;
}
else
{
lean_inc(v_f_852_);
lean_dec(v_x_816_);
v___x_854_ = lean_box(0);
v_isShared_855_ = v_isSharedCheck_860_;
goto v_resetjp_853_;
}
v_resetjp_853_:
{
lean_object* v___x_856_; lean_object* v___x_858_; 
v___x_856_ = lp_arrow__minimality__lib_StructuralInfo_encodeMonad___redArg(v_f_852_);
if (v_isShared_855_ == 0)
{
lean_ctor_set_tag(v___x_854_, 4);
lean_ctor_set(v___x_854_, 0, v___x_856_);
v___x_858_ = v___x_854_;
goto v_reusejp_857_;
}
else
{
lean_object* v_reuseFailAlloc_859_; 
v_reuseFailAlloc_859_ = lean_alloc_ctor(4, 1, 0);
lean_ctor_set(v_reuseFailAlloc_859_, 0, v___x_856_);
v___x_858_ = v_reuseFailAlloc_859_;
goto v_reusejp_857_;
}
v_reusejp_857_:
{
return v___x_858_;
}
}
}
case 6:
{
lean_object* v_f_861_; 
v_f_861_ = lean_ctor_get(v_x_816_, 0);
lean_inc_ref(v_f_861_);
lean_dec_ref_known(v_x_816_, 1);
v_x_816_ = v_f_861_;
goto _start;
}
default: 
{
lean_object* v_f_863_; lean_object* v_g_864_; 
v_f_863_ = lean_ctor_get(v_x_816_, 0);
lean_inc_ref(v_f_863_);
v_g_864_ = lean_ctor_get(v_x_816_, 1);
lean_inc_ref(v_g_864_);
lean_dec_ref(v_x_816_);
v_f_818_ = v_f_863_;
v_g_819_ = v_g_864_;
goto v___jp_817_;
}
}
v___jp_817_:
{
lean_object* v___x_820_; lean_object* v___x_821_; lean_object* v___x_822_; 
v___x_820_ = lp_arrow__minimality__lib_StructuralInfo_encodeMonad___redArg(v_f_818_);
v___x_821_ = lp_arrow__minimality__lib_StructuralInfo_encodeMonad___redArg(v_g_819_);
v___x_822_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v___x_822_, 0, v___x_820_);
lean_ctor_set(v___x_822_, 1, v___x_821_);
return v___x_822_;
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_encodeMonad(lean_object* v_Op_865_, lean_object* v_V_866_, lean_object* v_x_867_){
_start:
{
lean_object* v___x_868_; 
v___x_868_ = lp_arrow__minimality__lib_StructuralInfo_encodeMonad___redArg(v_x_867_);
return v___x_868_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_encodeTagged___redArg(lean_object* v_defaultLabel_869_, lean_object* v_x_870_){
_start:
{
switch(lean_obj_tag(v_x_870_))
{
case 0:
{
lean_object* v_v_871_; lean_object* v___x_873_; uint8_t v_isShared_874_; uint8_t v_isSharedCheck_878_; 
lean_dec(v_defaultLabel_869_);
v_v_871_ = lean_ctor_get(v_x_870_, 0);
v_isSharedCheck_878_ = !lean_is_exclusive(v_x_870_);
if (v_isSharedCheck_878_ == 0)
{
v___x_873_ = v_x_870_;
v_isShared_874_ = v_isSharedCheck_878_;
goto v_resetjp_872_;
}
else
{
lean_inc(v_v_871_);
lean_dec(v_x_870_);
v___x_873_ = lean_box(0);
v_isShared_874_ = v_isSharedCheck_878_;
goto v_resetjp_872_;
}
v_resetjp_872_:
{
lean_object* v___x_876_; 
if (v_isShared_874_ == 0)
{
v___x_876_ = v___x_873_;
goto v_reusejp_875_;
}
else
{
lean_object* v_reuseFailAlloc_877_; 
v_reuseFailAlloc_877_ = lean_alloc_ctor(0, 1, 0);
lean_ctor_set(v_reuseFailAlloc_877_, 0, v_v_871_);
v___x_876_ = v_reuseFailAlloc_877_;
goto v_reusejp_875_;
}
v_reusejp_875_:
{
return v___x_876_;
}
}
}
case 1:
{
lean_object* v_f_879_; lean_object* v_g_880_; lean_object* v___x_882_; uint8_t v_isShared_883_; uint8_t v_isSharedCheck_889_; 
v_f_879_ = lean_ctor_get(v_x_870_, 0);
v_g_880_ = lean_ctor_get(v_x_870_, 1);
v_isSharedCheck_889_ = !lean_is_exclusive(v_x_870_);
if (v_isSharedCheck_889_ == 0)
{
v___x_882_ = v_x_870_;
v_isShared_883_ = v_isSharedCheck_889_;
goto v_resetjp_881_;
}
else
{
lean_inc(v_g_880_);
lean_inc(v_f_879_);
lean_dec(v_x_870_);
v___x_882_ = lean_box(0);
v_isShared_883_ = v_isSharedCheck_889_;
goto v_resetjp_881_;
}
v_resetjp_881_:
{
lean_object* v___x_884_; lean_object* v___x_885_; lean_object* v___x_887_; 
lean_inc(v_defaultLabel_869_);
v___x_884_ = lp_arrow__minimality__lib_StructuralInfo_encodeTagged___redArg(v_defaultLabel_869_, v_f_879_);
v___x_885_ = lp_arrow__minimality__lib_StructuralInfo_encodeTagged___redArg(v_defaultLabel_869_, v_g_880_);
if (v_isShared_883_ == 0)
{
lean_ctor_set(v___x_882_, 1, v___x_885_);
lean_ctor_set(v___x_882_, 0, v___x_884_);
v___x_887_ = v___x_882_;
goto v_reusejp_886_;
}
else
{
lean_object* v_reuseFailAlloc_888_; 
v_reuseFailAlloc_888_ = lean_alloc_ctor(1, 2, 0);
lean_ctor_set(v_reuseFailAlloc_888_, 0, v___x_884_);
lean_ctor_set(v_reuseFailAlloc_888_, 1, v___x_885_);
v___x_887_ = v_reuseFailAlloc_888_;
goto v_reusejp_886_;
}
v_reusejp_886_:
{
return v___x_887_;
}
}
}
case 2:
{
lean_object* v_f_890_; lean_object* v_g_891_; lean_object* v___x_893_; uint8_t v_isShared_894_; uint8_t v_isSharedCheck_900_; 
v_f_890_ = lean_ctor_get(v_x_870_, 0);
v_g_891_ = lean_ctor_get(v_x_870_, 1);
v_isSharedCheck_900_ = !lean_is_exclusive(v_x_870_);
if (v_isSharedCheck_900_ == 0)
{
v___x_893_ = v_x_870_;
v_isShared_894_ = v_isSharedCheck_900_;
goto v_resetjp_892_;
}
else
{
lean_inc(v_g_891_);
lean_inc(v_f_890_);
lean_dec(v_x_870_);
v___x_893_ = lean_box(0);
v_isShared_894_ = v_isSharedCheck_900_;
goto v_resetjp_892_;
}
v_resetjp_892_:
{
lean_object* v___x_895_; lean_object* v___x_896_; lean_object* v___x_898_; 
lean_inc(v_defaultLabel_869_);
v___x_895_ = lp_arrow__minimality__lib_StructuralInfo_encodeTagged___redArg(v_defaultLabel_869_, v_f_890_);
v___x_896_ = lp_arrow__minimality__lib_StructuralInfo_encodeTagged___redArg(v_defaultLabel_869_, v_g_891_);
if (v_isShared_894_ == 0)
{
lean_ctor_set(v___x_893_, 1, v___x_896_);
lean_ctor_set(v___x_893_, 0, v___x_895_);
v___x_898_ = v___x_893_;
goto v_reusejp_897_;
}
else
{
lean_object* v_reuseFailAlloc_899_; 
v_reuseFailAlloc_899_ = lean_alloc_ctor(2, 2, 0);
lean_ctor_set(v_reuseFailAlloc_899_, 0, v___x_895_);
lean_ctor_set(v_reuseFailAlloc_899_, 1, v___x_896_);
v___x_898_ = v_reuseFailAlloc_899_;
goto v_reusejp_897_;
}
v_reusejp_897_:
{
return v___x_898_;
}
}
}
case 3:
{
lean_object* v_f_901_; lean_object* v_g_902_; lean_object* v___x_904_; uint8_t v_isShared_905_; uint8_t v_isSharedCheck_911_; 
v_f_901_ = lean_ctor_get(v_x_870_, 0);
v_g_902_ = lean_ctor_get(v_x_870_, 1);
v_isSharedCheck_911_ = !lean_is_exclusive(v_x_870_);
if (v_isSharedCheck_911_ == 0)
{
v___x_904_ = v_x_870_;
v_isShared_905_ = v_isSharedCheck_911_;
goto v_resetjp_903_;
}
else
{
lean_inc(v_g_902_);
lean_inc(v_f_901_);
lean_dec(v_x_870_);
v___x_904_ = lean_box(0);
v_isShared_905_ = v_isSharedCheck_911_;
goto v_resetjp_903_;
}
v_resetjp_903_:
{
lean_object* v___x_906_; lean_object* v___x_907_; lean_object* v___x_909_; 
lean_inc(v_defaultLabel_869_);
v___x_906_ = lp_arrow__minimality__lib_StructuralInfo_encodeTagged___redArg(v_defaultLabel_869_, v_f_901_);
v___x_907_ = lp_arrow__minimality__lib_StructuralInfo_encodeTagged___redArg(v_defaultLabel_869_, v_g_902_);
if (v_isShared_905_ == 0)
{
lean_ctor_set(v___x_904_, 1, v___x_907_);
lean_ctor_set(v___x_904_, 0, v___x_906_);
v___x_909_ = v___x_904_;
goto v_reusejp_908_;
}
else
{
lean_object* v_reuseFailAlloc_910_; 
v_reuseFailAlloc_910_ = lean_alloc_ctor(3, 2, 0);
lean_ctor_set(v_reuseFailAlloc_910_, 0, v___x_906_);
lean_ctor_set(v_reuseFailAlloc_910_, 1, v___x_907_);
v___x_909_ = v_reuseFailAlloc_910_;
goto v_reusejp_908_;
}
v_reusejp_908_:
{
return v___x_909_;
}
}
}
case 4:
{
lean_object* v_o_912_; lean_object* v_k_913_; lean_object* v___x_914_; lean_object* v___x_915_; 
v_o_912_ = lean_ctor_get(v_x_870_, 0);
lean_inc(v_o_912_);
v_k_913_ = lean_ctor_get(v_x_870_, 1);
lean_inc_ref(v_k_913_);
lean_dec_ref_known(v_x_870_, 2);
lean_inc(v_defaultLabel_869_);
v___x_914_ = lp_arrow__minimality__lib_StructuralInfo_encodeTagged___redArg(v_defaultLabel_869_, v_k_913_);
v___x_915_ = lean_alloc_ctor(4, 3, 0);
lean_ctor_set(v___x_915_, 0, v_defaultLabel_869_);
lean_ctor_set(v___x_915_, 1, v_o_912_);
lean_ctor_set(v___x_915_, 2, v___x_914_);
return v___x_915_;
}
case 5:
{
lean_object* v_f_916_; lean_object* v___x_918_; uint8_t v_isShared_919_; uint8_t v_isSharedCheck_924_; 
v_f_916_ = lean_ctor_get(v_x_870_, 0);
v_isSharedCheck_924_ = !lean_is_exclusive(v_x_870_);
if (v_isSharedCheck_924_ == 0)
{
v___x_918_ = v_x_870_;
v_isShared_919_ = v_isSharedCheck_924_;
goto v_resetjp_917_;
}
else
{
lean_inc(v_f_916_);
lean_dec(v_x_870_);
v___x_918_ = lean_box(0);
v_isShared_919_ = v_isSharedCheck_924_;
goto v_resetjp_917_;
}
v_resetjp_917_:
{
lean_object* v___x_920_; lean_object* v___x_922_; 
v___x_920_ = lp_arrow__minimality__lib_StructuralInfo_encodeTagged___redArg(v_defaultLabel_869_, v_f_916_);
if (v_isShared_919_ == 0)
{
lean_ctor_set(v___x_918_, 0, v___x_920_);
v___x_922_ = v___x_918_;
goto v_reusejp_921_;
}
else
{
lean_object* v_reuseFailAlloc_923_; 
v_reuseFailAlloc_923_ = lean_alloc_ctor(5, 1, 0);
lean_ctor_set(v_reuseFailAlloc_923_, 0, v___x_920_);
v___x_922_ = v_reuseFailAlloc_923_;
goto v_reusejp_921_;
}
v_reusejp_921_:
{
return v___x_922_;
}
}
}
default: 
{
lean_object* v_f_925_; lean_object* v___x_927_; uint8_t v_isShared_928_; uint8_t v_isSharedCheck_933_; 
v_f_925_ = lean_ctor_get(v_x_870_, 0);
v_isSharedCheck_933_ = !lean_is_exclusive(v_x_870_);
if (v_isSharedCheck_933_ == 0)
{
v___x_927_ = v_x_870_;
v_isShared_928_ = v_isSharedCheck_933_;
goto v_resetjp_926_;
}
else
{
lean_inc(v_f_925_);
lean_dec(v_x_870_);
v___x_927_ = lean_box(0);
v_isShared_928_ = v_isSharedCheck_933_;
goto v_resetjp_926_;
}
v_resetjp_926_:
{
lean_object* v___x_929_; lean_object* v___x_931_; 
v___x_929_ = lp_arrow__minimality__lib_StructuralInfo_encodeTagged___redArg(v_defaultLabel_869_, v_f_925_);
if (v_isShared_928_ == 0)
{
lean_ctor_set(v___x_927_, 0, v___x_929_);
v___x_931_ = v___x_927_;
goto v_reusejp_930_;
}
else
{
lean_object* v_reuseFailAlloc_932_; 
v_reuseFailAlloc_932_ = lean_alloc_ctor(6, 1, 0);
lean_ctor_set(v_reuseFailAlloc_932_, 0, v___x_929_);
v___x_931_ = v_reuseFailAlloc_932_;
goto v_reusejp_930_;
}
v_reusejp_930_:
{
return v___x_931_;
}
}
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib_StructuralInfo_encodeTagged(lean_object* v_Op_934_, lean_object* v_V_935_, lean_object* v_Label_936_, lean_object* v_defaultLabel_937_, lean_object* v_x_938_){
_start:
{
lean_object* v___x_939_; 
v___x_939_ = lp_arrow__minimality__lib_StructuralInfo_encodeTagged___redArg(v_defaultLabel_937_, v_x_938_);
return v___x_939_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_StructuralInfo_0__StructuralInfo_holds_match__1_splitter___redArg(uint8_t v_x_940_, lean_object* v_x_941_, lean_object* v_h__1_942_, lean_object* v_h__2_943_, lean_object* v_h__3_944_, lean_object* v_h__4_945_, lean_object* v_h__5_946_, lean_object* v_h__6_947_, lean_object* v_h__7_948_){
_start:
{
switch(v_x_940_)
{
case 0:
{
lean_object* v___x_949_; 
lean_dec(v_h__7_948_);
lean_dec(v_h__6_947_);
lean_dec(v_h__5_946_);
lean_dec(v_h__4_945_);
lean_dec(v_h__3_944_);
lean_dec(v_h__2_943_);
v___x_949_ = lean_apply_1(v_h__1_942_, v_x_941_);
return v___x_949_;
}
case 1:
{
lean_object* v___x_950_; 
lean_dec(v_h__7_948_);
lean_dec(v_h__6_947_);
lean_dec(v_h__5_946_);
lean_dec(v_h__4_945_);
lean_dec(v_h__3_944_);
lean_dec(v_h__1_942_);
v___x_950_ = lean_apply_1(v_h__2_943_, v_x_941_);
return v___x_950_;
}
case 2:
{
lean_object* v___x_951_; 
lean_dec(v_h__7_948_);
lean_dec(v_h__6_947_);
lean_dec(v_h__5_946_);
lean_dec(v_h__4_945_);
lean_dec(v_h__2_943_);
lean_dec(v_h__1_942_);
v___x_951_ = lean_apply_1(v_h__3_944_, v_x_941_);
return v___x_951_;
}
case 3:
{
lean_object* v___x_952_; 
lean_dec(v_h__7_948_);
lean_dec(v_h__6_947_);
lean_dec(v_h__5_946_);
lean_dec(v_h__3_944_);
lean_dec(v_h__2_943_);
lean_dec(v_h__1_942_);
v___x_952_ = lean_apply_1(v_h__4_945_, v_x_941_);
return v___x_952_;
}
case 4:
{
lean_object* v___x_953_; 
lean_dec(v_h__7_948_);
lean_dec(v_h__6_947_);
lean_dec(v_h__4_945_);
lean_dec(v_h__3_944_);
lean_dec(v_h__2_943_);
lean_dec(v_h__1_942_);
v___x_953_ = lean_apply_1(v_h__5_946_, v_x_941_);
return v___x_953_;
}
case 5:
{
lean_object* v___x_954_; 
lean_dec(v_h__7_948_);
lean_dec(v_h__5_946_);
lean_dec(v_h__4_945_);
lean_dec(v_h__3_944_);
lean_dec(v_h__2_943_);
lean_dec(v_h__1_942_);
v___x_954_ = lean_apply_1(v_h__6_947_, v_x_941_);
return v___x_954_;
}
default: 
{
lean_object* v___x_955_; 
lean_dec(v_h__6_947_);
lean_dec(v_h__5_946_);
lean_dec(v_h__4_945_);
lean_dec(v_h__3_944_);
lean_dec(v_h__2_943_);
lean_dec(v_h__1_942_);
v___x_955_ = lean_apply_1(v_h__7_948_, v_x_941_);
return v___x_955_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_StructuralInfo_0__StructuralInfo_holds_match__1_splitter___redArg___boxed(lean_object* v_x_956_, lean_object* v_x_957_, lean_object* v_h__1_958_, lean_object* v_h__2_959_, lean_object* v_h__3_960_, lean_object* v_h__4_961_, lean_object* v_h__5_962_, lean_object* v_h__6_963_, lean_object* v_h__7_964_){
_start:
{
uint8_t v_x_52__boxed_965_; lean_object* v_res_966_; 
v_x_52__boxed_965_ = lean_unbox(v_x_956_);
v_res_966_ = lp_arrow__minimality__lib___private_ArrowMinimalityLib_StructuralInfo_0__StructuralInfo_holds_match__1_splitter___redArg(v_x_52__boxed_965_, v_x_957_, v_h__1_958_, v_h__2_959_, v_h__3_960_, v_h__4_961_, v_h__5_962_, v_h__6_963_, v_h__7_964_);
return v_res_966_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_StructuralInfo_0__StructuralInfo_holds_match__1_splitter(lean_object* v_Op_967_, lean_object* v_V_968_, lean_object* v_motive_969_, uint8_t v_x_970_, lean_object* v_x_971_, lean_object* v_h__1_972_, lean_object* v_h__2_973_, lean_object* v_h__3_974_, lean_object* v_h__4_975_, lean_object* v_h__5_976_, lean_object* v_h__6_977_, lean_object* v_h__7_978_){
_start:
{
switch(v_x_970_)
{
case 0:
{
lean_object* v___x_979_; 
lean_dec(v_h__7_978_);
lean_dec(v_h__6_977_);
lean_dec(v_h__5_976_);
lean_dec(v_h__4_975_);
lean_dec(v_h__3_974_);
lean_dec(v_h__2_973_);
v___x_979_ = lean_apply_1(v_h__1_972_, v_x_971_);
return v___x_979_;
}
case 1:
{
lean_object* v___x_980_; 
lean_dec(v_h__7_978_);
lean_dec(v_h__6_977_);
lean_dec(v_h__5_976_);
lean_dec(v_h__4_975_);
lean_dec(v_h__3_974_);
lean_dec(v_h__1_972_);
v___x_980_ = lean_apply_1(v_h__2_973_, v_x_971_);
return v___x_980_;
}
case 2:
{
lean_object* v___x_981_; 
lean_dec(v_h__7_978_);
lean_dec(v_h__6_977_);
lean_dec(v_h__5_976_);
lean_dec(v_h__4_975_);
lean_dec(v_h__2_973_);
lean_dec(v_h__1_972_);
v___x_981_ = lean_apply_1(v_h__3_974_, v_x_971_);
return v___x_981_;
}
case 3:
{
lean_object* v___x_982_; 
lean_dec(v_h__7_978_);
lean_dec(v_h__6_977_);
lean_dec(v_h__5_976_);
lean_dec(v_h__3_974_);
lean_dec(v_h__2_973_);
lean_dec(v_h__1_972_);
v___x_982_ = lean_apply_1(v_h__4_975_, v_x_971_);
return v___x_982_;
}
case 4:
{
lean_object* v___x_983_; 
lean_dec(v_h__7_978_);
lean_dec(v_h__6_977_);
lean_dec(v_h__4_975_);
lean_dec(v_h__3_974_);
lean_dec(v_h__2_973_);
lean_dec(v_h__1_972_);
v___x_983_ = lean_apply_1(v_h__5_976_, v_x_971_);
return v___x_983_;
}
case 5:
{
lean_object* v___x_984_; 
lean_dec(v_h__7_978_);
lean_dec(v_h__5_976_);
lean_dec(v_h__4_975_);
lean_dec(v_h__3_974_);
lean_dec(v_h__2_973_);
lean_dec(v_h__1_972_);
v___x_984_ = lean_apply_1(v_h__6_977_, v_x_971_);
return v___x_984_;
}
default: 
{
lean_object* v___x_985_; 
lean_dec(v_h__6_977_);
lean_dec(v_h__5_976_);
lean_dec(v_h__4_975_);
lean_dec(v_h__3_974_);
lean_dec(v_h__2_973_);
lean_dec(v_h__1_972_);
v___x_985_ = lean_apply_1(v_h__7_978_, v_x_971_);
return v___x_985_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_StructuralInfo_0__StructuralInfo_holds_match__1_splitter___boxed(lean_object* v_Op_986_, lean_object* v_V_987_, lean_object* v_motive_988_, lean_object* v_x_989_, lean_object* v_x_990_, lean_object* v_h__1_991_, lean_object* v_h__2_992_, lean_object* v_h__3_993_, lean_object* v_h__4_994_, lean_object* v_h__5_995_, lean_object* v_h__6_996_, lean_object* v_h__7_997_){
_start:
{
uint8_t v_x_72__boxed_998_; lean_object* v_res_999_; 
v_x_72__boxed_998_ = lean_unbox(v_x_989_);
v_res_999_ = lp_arrow__minimality__lib___private_ArrowMinimalityLib_StructuralInfo_0__StructuralInfo_holds_match__1_splitter(v_Op_986_, v_V_987_, v_motive_988_, v_x_72__boxed_998_, v_x_990_, v_h__1_991_, v_h__2_992_, v_h__3_993_, v_h__4_994_, v_h__5_995_, v_h__6_996_, v_h__7_997_);
return v_res_999_;
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_StructuralInfo_0__StructuralInfo_containsFanout_match__1_splitter___redArg(lean_object* v_x_1000_, lean_object* v_h__1_1001_, lean_object* v_h__2_1002_, lean_object* v_h__3_1003_, lean_object* v_h__4_1004_, lean_object* v_h__5_1005_, lean_object* v_h__6_1006_, lean_object* v_h__7_1007_){
_start:
{
switch(lean_obj_tag(v_x_1000_))
{
case 0:
{
lean_object* v_v_1008_; lean_object* v___x_1009_; 
lean_dec(v_h__6_1006_);
lean_dec(v_h__5_1005_);
lean_dec(v_h__4_1004_);
lean_dec(v_h__3_1003_);
lean_dec(v_h__2_1002_);
lean_dec(v_h__1_1001_);
v_v_1008_ = lean_ctor_get(v_x_1000_, 0);
lean_inc(v_v_1008_);
lean_dec_ref_known(v_x_1000_, 1);
v___x_1009_ = lean_apply_1(v_h__7_1007_, v_v_1008_);
return v___x_1009_;
}
case 1:
{
lean_object* v_f_1010_; lean_object* v_g_1011_; lean_object* v___x_1012_; 
lean_dec(v_h__7_1007_);
lean_dec(v_h__6_1006_);
lean_dec(v_h__5_1005_);
lean_dec(v_h__4_1004_);
lean_dec(v_h__3_1003_);
lean_dec(v_h__1_1001_);
v_f_1010_ = lean_ctor_get(v_x_1000_, 0);
lean_inc_ref(v_f_1010_);
v_g_1011_ = lean_ctor_get(v_x_1000_, 1);
lean_inc_ref(v_g_1011_);
lean_dec_ref_known(v_x_1000_, 2);
v___x_1012_ = lean_apply_2(v_h__2_1002_, v_f_1010_, v_g_1011_);
return v___x_1012_;
}
case 2:
{
lean_object* v_f_1013_; lean_object* v_g_1014_; lean_object* v___x_1015_; 
lean_dec(v_h__7_1007_);
lean_dec(v_h__6_1006_);
lean_dec(v_h__5_1005_);
lean_dec(v_h__4_1004_);
lean_dec(v_h__3_1003_);
lean_dec(v_h__2_1002_);
v_f_1013_ = lean_ctor_get(v_x_1000_, 0);
lean_inc_ref(v_f_1013_);
v_g_1014_ = lean_ctor_get(v_x_1000_, 1);
lean_inc_ref(v_g_1014_);
lean_dec_ref_known(v_x_1000_, 2);
v___x_1015_ = lean_apply_2(v_h__1_1001_, v_f_1013_, v_g_1014_);
return v___x_1015_;
}
case 3:
{
lean_object* v_f_1016_; lean_object* v_g_1017_; lean_object* v___x_1018_; 
lean_dec(v_h__7_1007_);
lean_dec(v_h__6_1006_);
lean_dec(v_h__5_1005_);
lean_dec(v_h__4_1004_);
lean_dec(v_h__2_1002_);
lean_dec(v_h__1_1001_);
v_f_1016_ = lean_ctor_get(v_x_1000_, 0);
lean_inc_ref(v_f_1016_);
v_g_1017_ = lean_ctor_get(v_x_1000_, 1);
lean_inc_ref(v_g_1017_);
lean_dec_ref_known(v_x_1000_, 2);
v___x_1018_ = lean_apply_2(v_h__3_1003_, v_f_1016_, v_g_1017_);
return v___x_1018_;
}
case 4:
{
lean_object* v_o_1019_; lean_object* v_k_1020_; lean_object* v___x_1021_; 
lean_dec(v_h__7_1007_);
lean_dec(v_h__6_1006_);
lean_dec(v_h__5_1005_);
lean_dec(v_h__3_1003_);
lean_dec(v_h__2_1002_);
lean_dec(v_h__1_1001_);
v_o_1019_ = lean_ctor_get(v_x_1000_, 0);
lean_inc(v_o_1019_);
v_k_1020_ = lean_ctor_get(v_x_1000_, 1);
lean_inc_ref(v_k_1020_);
lean_dec_ref_known(v_x_1000_, 2);
v___x_1021_ = lean_apply_2(v_h__4_1004_, v_o_1019_, v_k_1020_);
return v___x_1021_;
}
case 5:
{
lean_object* v_f_1022_; lean_object* v___x_1023_; 
lean_dec(v_h__7_1007_);
lean_dec(v_h__6_1006_);
lean_dec(v_h__4_1004_);
lean_dec(v_h__3_1003_);
lean_dec(v_h__2_1002_);
lean_dec(v_h__1_1001_);
v_f_1022_ = lean_ctor_get(v_x_1000_, 0);
lean_inc_ref(v_f_1022_);
lean_dec_ref_known(v_x_1000_, 1);
v___x_1023_ = lean_apply_1(v_h__5_1005_, v_f_1022_);
return v___x_1023_;
}
default: 
{
lean_object* v_f_1024_; lean_object* v___x_1025_; 
lean_dec(v_h__7_1007_);
lean_dec(v_h__5_1005_);
lean_dec(v_h__4_1004_);
lean_dec(v_h__3_1003_);
lean_dec(v_h__2_1002_);
lean_dec(v_h__1_1001_);
v_f_1024_ = lean_ctor_get(v_x_1000_, 0);
lean_inc_ref(v_f_1024_);
lean_dec_ref_known(v_x_1000_, 1);
v___x_1025_ = lean_apply_1(v_h__6_1006_, v_f_1024_);
return v___x_1025_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_StructuralInfo_0__StructuralInfo_containsFanout_match__1_splitter(lean_object* v_Op_1026_, lean_object* v_V_1027_, lean_object* v_motive_1028_, lean_object* v_x_1029_, lean_object* v_h__1_1030_, lean_object* v_h__2_1031_, lean_object* v_h__3_1032_, lean_object* v_h__4_1033_, lean_object* v_h__5_1034_, lean_object* v_h__6_1035_, lean_object* v_h__7_1036_){
_start:
{
switch(lean_obj_tag(v_x_1029_))
{
case 0:
{
lean_object* v_v_1037_; lean_object* v___x_1038_; 
lean_dec(v_h__6_1035_);
lean_dec(v_h__5_1034_);
lean_dec(v_h__4_1033_);
lean_dec(v_h__3_1032_);
lean_dec(v_h__2_1031_);
lean_dec(v_h__1_1030_);
v_v_1037_ = lean_ctor_get(v_x_1029_, 0);
lean_inc(v_v_1037_);
lean_dec_ref_known(v_x_1029_, 1);
v___x_1038_ = lean_apply_1(v_h__7_1036_, v_v_1037_);
return v___x_1038_;
}
case 1:
{
lean_object* v_f_1039_; lean_object* v_g_1040_; lean_object* v___x_1041_; 
lean_dec(v_h__7_1036_);
lean_dec(v_h__6_1035_);
lean_dec(v_h__5_1034_);
lean_dec(v_h__4_1033_);
lean_dec(v_h__3_1032_);
lean_dec(v_h__1_1030_);
v_f_1039_ = lean_ctor_get(v_x_1029_, 0);
lean_inc_ref(v_f_1039_);
v_g_1040_ = lean_ctor_get(v_x_1029_, 1);
lean_inc_ref(v_g_1040_);
lean_dec_ref_known(v_x_1029_, 2);
v___x_1041_ = lean_apply_2(v_h__2_1031_, v_f_1039_, v_g_1040_);
return v___x_1041_;
}
case 2:
{
lean_object* v_f_1042_; lean_object* v_g_1043_; lean_object* v___x_1044_; 
lean_dec(v_h__7_1036_);
lean_dec(v_h__6_1035_);
lean_dec(v_h__5_1034_);
lean_dec(v_h__4_1033_);
lean_dec(v_h__3_1032_);
lean_dec(v_h__2_1031_);
v_f_1042_ = lean_ctor_get(v_x_1029_, 0);
lean_inc_ref(v_f_1042_);
v_g_1043_ = lean_ctor_get(v_x_1029_, 1);
lean_inc_ref(v_g_1043_);
lean_dec_ref_known(v_x_1029_, 2);
v___x_1044_ = lean_apply_2(v_h__1_1030_, v_f_1042_, v_g_1043_);
return v___x_1044_;
}
case 3:
{
lean_object* v_f_1045_; lean_object* v_g_1046_; lean_object* v___x_1047_; 
lean_dec(v_h__7_1036_);
lean_dec(v_h__6_1035_);
lean_dec(v_h__5_1034_);
lean_dec(v_h__4_1033_);
lean_dec(v_h__2_1031_);
lean_dec(v_h__1_1030_);
v_f_1045_ = lean_ctor_get(v_x_1029_, 0);
lean_inc_ref(v_f_1045_);
v_g_1046_ = lean_ctor_get(v_x_1029_, 1);
lean_inc_ref(v_g_1046_);
lean_dec_ref_known(v_x_1029_, 2);
v___x_1047_ = lean_apply_2(v_h__3_1032_, v_f_1045_, v_g_1046_);
return v___x_1047_;
}
case 4:
{
lean_object* v_o_1048_; lean_object* v_k_1049_; lean_object* v___x_1050_; 
lean_dec(v_h__7_1036_);
lean_dec(v_h__6_1035_);
lean_dec(v_h__5_1034_);
lean_dec(v_h__3_1032_);
lean_dec(v_h__2_1031_);
lean_dec(v_h__1_1030_);
v_o_1048_ = lean_ctor_get(v_x_1029_, 0);
lean_inc(v_o_1048_);
v_k_1049_ = lean_ctor_get(v_x_1029_, 1);
lean_inc_ref(v_k_1049_);
lean_dec_ref_known(v_x_1029_, 2);
v___x_1050_ = lean_apply_2(v_h__4_1033_, v_o_1048_, v_k_1049_);
return v___x_1050_;
}
case 5:
{
lean_object* v_f_1051_; lean_object* v___x_1052_; 
lean_dec(v_h__7_1036_);
lean_dec(v_h__6_1035_);
lean_dec(v_h__4_1033_);
lean_dec(v_h__3_1032_);
lean_dec(v_h__2_1031_);
lean_dec(v_h__1_1030_);
v_f_1051_ = lean_ctor_get(v_x_1029_, 0);
lean_inc_ref(v_f_1051_);
lean_dec_ref_known(v_x_1029_, 1);
v___x_1052_ = lean_apply_1(v_h__5_1034_, v_f_1051_);
return v___x_1052_;
}
default: 
{
lean_object* v_f_1053_; lean_object* v___x_1054_; 
lean_dec(v_h__7_1036_);
lean_dec(v_h__5_1034_);
lean_dec(v_h__4_1033_);
lean_dec(v_h__3_1032_);
lean_dec(v_h__2_1031_);
lean_dec(v_h__1_1030_);
v_f_1053_ = lean_ctor_get(v_x_1029_, 0);
lean_inc_ref(v_f_1053_);
lean_dec_ref_known(v_x_1029_, 1);
v___x_1054_ = lean_apply_1(v_h__6_1035_, v_f_1053_);
return v___x_1054_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_StructuralInfo_0__StructuralInfo_containsChoice_match__1_splitter___redArg(lean_object* v_x_1055_, lean_object* v_h__1_1056_, lean_object* v_h__2_1057_, lean_object* v_h__3_1058_, lean_object* v_h__4_1059_, lean_object* v_h__5_1060_, lean_object* v_h__6_1061_, lean_object* v_h__7_1062_){
_start:
{
switch(lean_obj_tag(v_x_1055_))
{
case 0:
{
lean_object* v_v_1063_; lean_object* v___x_1064_; 
lean_dec(v_h__6_1061_);
lean_dec(v_h__5_1060_);
lean_dec(v_h__4_1059_);
lean_dec(v_h__3_1058_);
lean_dec(v_h__2_1057_);
lean_dec(v_h__1_1056_);
v_v_1063_ = lean_ctor_get(v_x_1055_, 0);
lean_inc(v_v_1063_);
lean_dec_ref_known(v_x_1055_, 1);
v___x_1064_ = lean_apply_1(v_h__7_1062_, v_v_1063_);
return v___x_1064_;
}
case 1:
{
lean_object* v_f_1065_; lean_object* v_g_1066_; lean_object* v___x_1067_; 
lean_dec(v_h__7_1062_);
lean_dec(v_h__6_1061_);
lean_dec(v_h__5_1060_);
lean_dec(v_h__4_1059_);
lean_dec(v_h__3_1058_);
lean_dec(v_h__1_1056_);
v_f_1065_ = lean_ctor_get(v_x_1055_, 0);
lean_inc_ref(v_f_1065_);
v_g_1066_ = lean_ctor_get(v_x_1055_, 1);
lean_inc_ref(v_g_1066_);
lean_dec_ref_known(v_x_1055_, 2);
v___x_1067_ = lean_apply_2(v_h__2_1057_, v_f_1065_, v_g_1066_);
return v___x_1067_;
}
case 2:
{
lean_object* v_f_1068_; lean_object* v_g_1069_; lean_object* v___x_1070_; 
lean_dec(v_h__7_1062_);
lean_dec(v_h__6_1061_);
lean_dec(v_h__5_1060_);
lean_dec(v_h__4_1059_);
lean_dec(v_h__2_1057_);
lean_dec(v_h__1_1056_);
v_f_1068_ = lean_ctor_get(v_x_1055_, 0);
lean_inc_ref(v_f_1068_);
v_g_1069_ = lean_ctor_get(v_x_1055_, 1);
lean_inc_ref(v_g_1069_);
lean_dec_ref_known(v_x_1055_, 2);
v___x_1070_ = lean_apply_2(v_h__3_1058_, v_f_1068_, v_g_1069_);
return v___x_1070_;
}
case 3:
{
lean_object* v_f_1071_; lean_object* v_g_1072_; lean_object* v___x_1073_; 
lean_dec(v_h__7_1062_);
lean_dec(v_h__6_1061_);
lean_dec(v_h__5_1060_);
lean_dec(v_h__4_1059_);
lean_dec(v_h__3_1058_);
lean_dec(v_h__2_1057_);
v_f_1071_ = lean_ctor_get(v_x_1055_, 0);
lean_inc_ref(v_f_1071_);
v_g_1072_ = lean_ctor_get(v_x_1055_, 1);
lean_inc_ref(v_g_1072_);
lean_dec_ref_known(v_x_1055_, 2);
v___x_1073_ = lean_apply_2(v_h__1_1056_, v_f_1071_, v_g_1072_);
return v___x_1073_;
}
case 4:
{
lean_object* v_o_1074_; lean_object* v_k_1075_; lean_object* v___x_1076_; 
lean_dec(v_h__7_1062_);
lean_dec(v_h__6_1061_);
lean_dec(v_h__5_1060_);
lean_dec(v_h__3_1058_);
lean_dec(v_h__2_1057_);
lean_dec(v_h__1_1056_);
v_o_1074_ = lean_ctor_get(v_x_1055_, 0);
lean_inc(v_o_1074_);
v_k_1075_ = lean_ctor_get(v_x_1055_, 1);
lean_inc_ref(v_k_1075_);
lean_dec_ref_known(v_x_1055_, 2);
v___x_1076_ = lean_apply_2(v_h__4_1059_, v_o_1074_, v_k_1075_);
return v___x_1076_;
}
case 5:
{
lean_object* v_f_1077_; lean_object* v___x_1078_; 
lean_dec(v_h__7_1062_);
lean_dec(v_h__6_1061_);
lean_dec(v_h__4_1059_);
lean_dec(v_h__3_1058_);
lean_dec(v_h__2_1057_);
lean_dec(v_h__1_1056_);
v_f_1077_ = lean_ctor_get(v_x_1055_, 0);
lean_inc_ref(v_f_1077_);
lean_dec_ref_known(v_x_1055_, 1);
v___x_1078_ = lean_apply_1(v_h__5_1060_, v_f_1077_);
return v___x_1078_;
}
default: 
{
lean_object* v_f_1079_; lean_object* v___x_1080_; 
lean_dec(v_h__7_1062_);
lean_dec(v_h__5_1060_);
lean_dec(v_h__4_1059_);
lean_dec(v_h__3_1058_);
lean_dec(v_h__2_1057_);
lean_dec(v_h__1_1056_);
v_f_1079_ = lean_ctor_get(v_x_1055_, 0);
lean_inc_ref(v_f_1079_);
lean_dec_ref_known(v_x_1055_, 1);
v___x_1080_ = lean_apply_1(v_h__6_1061_, v_f_1079_);
return v___x_1080_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_StructuralInfo_0__StructuralInfo_containsChoice_match__1_splitter(lean_object* v_Op_1081_, lean_object* v_V_1082_, lean_object* v_motive_1083_, lean_object* v_x_1084_, lean_object* v_h__1_1085_, lean_object* v_h__2_1086_, lean_object* v_h__3_1087_, lean_object* v_h__4_1088_, lean_object* v_h__5_1089_, lean_object* v_h__6_1090_, lean_object* v_h__7_1091_){
_start:
{
switch(lean_obj_tag(v_x_1084_))
{
case 0:
{
lean_object* v_v_1092_; lean_object* v___x_1093_; 
lean_dec(v_h__6_1090_);
lean_dec(v_h__5_1089_);
lean_dec(v_h__4_1088_);
lean_dec(v_h__3_1087_);
lean_dec(v_h__2_1086_);
lean_dec(v_h__1_1085_);
v_v_1092_ = lean_ctor_get(v_x_1084_, 0);
lean_inc(v_v_1092_);
lean_dec_ref_known(v_x_1084_, 1);
v___x_1093_ = lean_apply_1(v_h__7_1091_, v_v_1092_);
return v___x_1093_;
}
case 1:
{
lean_object* v_f_1094_; lean_object* v_g_1095_; lean_object* v___x_1096_; 
lean_dec(v_h__7_1091_);
lean_dec(v_h__6_1090_);
lean_dec(v_h__5_1089_);
lean_dec(v_h__4_1088_);
lean_dec(v_h__3_1087_);
lean_dec(v_h__1_1085_);
v_f_1094_ = lean_ctor_get(v_x_1084_, 0);
lean_inc_ref(v_f_1094_);
v_g_1095_ = lean_ctor_get(v_x_1084_, 1);
lean_inc_ref(v_g_1095_);
lean_dec_ref_known(v_x_1084_, 2);
v___x_1096_ = lean_apply_2(v_h__2_1086_, v_f_1094_, v_g_1095_);
return v___x_1096_;
}
case 2:
{
lean_object* v_f_1097_; lean_object* v_g_1098_; lean_object* v___x_1099_; 
lean_dec(v_h__7_1091_);
lean_dec(v_h__6_1090_);
lean_dec(v_h__5_1089_);
lean_dec(v_h__4_1088_);
lean_dec(v_h__2_1086_);
lean_dec(v_h__1_1085_);
v_f_1097_ = lean_ctor_get(v_x_1084_, 0);
lean_inc_ref(v_f_1097_);
v_g_1098_ = lean_ctor_get(v_x_1084_, 1);
lean_inc_ref(v_g_1098_);
lean_dec_ref_known(v_x_1084_, 2);
v___x_1099_ = lean_apply_2(v_h__3_1087_, v_f_1097_, v_g_1098_);
return v___x_1099_;
}
case 3:
{
lean_object* v_f_1100_; lean_object* v_g_1101_; lean_object* v___x_1102_; 
lean_dec(v_h__7_1091_);
lean_dec(v_h__6_1090_);
lean_dec(v_h__5_1089_);
lean_dec(v_h__4_1088_);
lean_dec(v_h__3_1087_);
lean_dec(v_h__2_1086_);
v_f_1100_ = lean_ctor_get(v_x_1084_, 0);
lean_inc_ref(v_f_1100_);
v_g_1101_ = lean_ctor_get(v_x_1084_, 1);
lean_inc_ref(v_g_1101_);
lean_dec_ref_known(v_x_1084_, 2);
v___x_1102_ = lean_apply_2(v_h__1_1085_, v_f_1100_, v_g_1101_);
return v___x_1102_;
}
case 4:
{
lean_object* v_o_1103_; lean_object* v_k_1104_; lean_object* v___x_1105_; 
lean_dec(v_h__7_1091_);
lean_dec(v_h__6_1090_);
lean_dec(v_h__5_1089_);
lean_dec(v_h__3_1087_);
lean_dec(v_h__2_1086_);
lean_dec(v_h__1_1085_);
v_o_1103_ = lean_ctor_get(v_x_1084_, 0);
lean_inc(v_o_1103_);
v_k_1104_ = lean_ctor_get(v_x_1084_, 1);
lean_inc_ref(v_k_1104_);
lean_dec_ref_known(v_x_1084_, 2);
v___x_1105_ = lean_apply_2(v_h__4_1088_, v_o_1103_, v_k_1104_);
return v___x_1105_;
}
case 5:
{
lean_object* v_f_1106_; lean_object* v___x_1107_; 
lean_dec(v_h__7_1091_);
lean_dec(v_h__6_1090_);
lean_dec(v_h__4_1088_);
lean_dec(v_h__3_1087_);
lean_dec(v_h__2_1086_);
lean_dec(v_h__1_1085_);
v_f_1106_ = lean_ctor_get(v_x_1084_, 0);
lean_inc_ref(v_f_1106_);
lean_dec_ref_known(v_x_1084_, 1);
v___x_1107_ = lean_apply_1(v_h__5_1089_, v_f_1106_);
return v___x_1107_;
}
default: 
{
lean_object* v_f_1108_; lean_object* v___x_1109_; 
lean_dec(v_h__7_1091_);
lean_dec(v_h__5_1089_);
lean_dec(v_h__4_1088_);
lean_dec(v_h__3_1087_);
lean_dec(v_h__2_1086_);
lean_dec(v_h__1_1085_);
v_f_1108_ = lean_ctor_get(v_x_1084_, 0);
lean_inc_ref(v_f_1108_);
lean_dec_ref_known(v_x_1084_, 1);
v___x_1109_ = lean_apply_1(v_h__6_1090_, v_f_1108_);
return v___x_1109_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_StructuralInfo_0__StructuralInfo_containsLoop_match__1_splitter___redArg(lean_object* v_x_1110_, lean_object* v_h__1_1111_, lean_object* v_h__2_1112_, lean_object* v_h__3_1113_, lean_object* v_h__4_1114_, lean_object* v_h__5_1115_, lean_object* v_h__6_1116_, lean_object* v_h__7_1117_){
_start:
{
switch(lean_obj_tag(v_x_1110_))
{
case 0:
{
lean_object* v_v_1118_; lean_object* v___x_1119_; 
lean_dec(v_h__6_1116_);
lean_dec(v_h__5_1115_);
lean_dec(v_h__4_1114_);
lean_dec(v_h__3_1113_);
lean_dec(v_h__2_1112_);
lean_dec(v_h__1_1111_);
v_v_1118_ = lean_ctor_get(v_x_1110_, 0);
lean_inc(v_v_1118_);
lean_dec_ref_known(v_x_1110_, 1);
v___x_1119_ = lean_apply_1(v_h__7_1117_, v_v_1118_);
return v___x_1119_;
}
case 1:
{
lean_object* v_f_1120_; lean_object* v_g_1121_; lean_object* v___x_1122_; 
lean_dec(v_h__7_1117_);
lean_dec(v_h__6_1116_);
lean_dec(v_h__5_1115_);
lean_dec(v_h__4_1114_);
lean_dec(v_h__3_1113_);
lean_dec(v_h__1_1111_);
v_f_1120_ = lean_ctor_get(v_x_1110_, 0);
lean_inc_ref(v_f_1120_);
v_g_1121_ = lean_ctor_get(v_x_1110_, 1);
lean_inc_ref(v_g_1121_);
lean_dec_ref_known(v_x_1110_, 2);
v___x_1122_ = lean_apply_2(v_h__2_1112_, v_f_1120_, v_g_1121_);
return v___x_1122_;
}
case 2:
{
lean_object* v_f_1123_; lean_object* v_g_1124_; lean_object* v___x_1125_; 
lean_dec(v_h__7_1117_);
lean_dec(v_h__6_1116_);
lean_dec(v_h__5_1115_);
lean_dec(v_h__4_1114_);
lean_dec(v_h__2_1112_);
lean_dec(v_h__1_1111_);
v_f_1123_ = lean_ctor_get(v_x_1110_, 0);
lean_inc_ref(v_f_1123_);
v_g_1124_ = lean_ctor_get(v_x_1110_, 1);
lean_inc_ref(v_g_1124_);
lean_dec_ref_known(v_x_1110_, 2);
v___x_1125_ = lean_apply_2(v_h__3_1113_, v_f_1123_, v_g_1124_);
return v___x_1125_;
}
case 3:
{
lean_object* v_f_1126_; lean_object* v_g_1127_; lean_object* v___x_1128_; 
lean_dec(v_h__7_1117_);
lean_dec(v_h__6_1116_);
lean_dec(v_h__5_1115_);
lean_dec(v_h__3_1113_);
lean_dec(v_h__2_1112_);
lean_dec(v_h__1_1111_);
v_f_1126_ = lean_ctor_get(v_x_1110_, 0);
lean_inc_ref(v_f_1126_);
v_g_1127_ = lean_ctor_get(v_x_1110_, 1);
lean_inc_ref(v_g_1127_);
lean_dec_ref_known(v_x_1110_, 2);
v___x_1128_ = lean_apply_2(v_h__4_1114_, v_f_1126_, v_g_1127_);
return v___x_1128_;
}
case 4:
{
lean_object* v_o_1129_; lean_object* v_k_1130_; lean_object* v___x_1131_; 
lean_dec(v_h__7_1117_);
lean_dec(v_h__6_1116_);
lean_dec(v_h__4_1114_);
lean_dec(v_h__3_1113_);
lean_dec(v_h__2_1112_);
lean_dec(v_h__1_1111_);
v_o_1129_ = lean_ctor_get(v_x_1110_, 0);
lean_inc(v_o_1129_);
v_k_1130_ = lean_ctor_get(v_x_1110_, 1);
lean_inc_ref(v_k_1130_);
lean_dec_ref_known(v_x_1110_, 2);
v___x_1131_ = lean_apply_2(v_h__5_1115_, v_o_1129_, v_k_1130_);
return v___x_1131_;
}
case 5:
{
lean_object* v_f_1132_; lean_object* v___x_1133_; 
lean_dec(v_h__7_1117_);
lean_dec(v_h__6_1116_);
lean_dec(v_h__5_1115_);
lean_dec(v_h__4_1114_);
lean_dec(v_h__3_1113_);
lean_dec(v_h__2_1112_);
v_f_1132_ = lean_ctor_get(v_x_1110_, 0);
lean_inc_ref(v_f_1132_);
lean_dec_ref_known(v_x_1110_, 1);
v___x_1133_ = lean_apply_1(v_h__1_1111_, v_f_1132_);
return v___x_1133_;
}
default: 
{
lean_object* v_f_1134_; lean_object* v___x_1135_; 
lean_dec(v_h__7_1117_);
lean_dec(v_h__5_1115_);
lean_dec(v_h__4_1114_);
lean_dec(v_h__3_1113_);
lean_dec(v_h__2_1112_);
lean_dec(v_h__1_1111_);
v_f_1134_ = lean_ctor_get(v_x_1110_, 0);
lean_inc_ref(v_f_1134_);
lean_dec_ref_known(v_x_1110_, 1);
v___x_1135_ = lean_apply_1(v_h__6_1116_, v_f_1134_);
return v___x_1135_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_StructuralInfo_0__StructuralInfo_containsLoop_match__1_splitter(lean_object* v_Op_1136_, lean_object* v_V_1137_, lean_object* v_motive_1138_, lean_object* v_x_1139_, lean_object* v_h__1_1140_, lean_object* v_h__2_1141_, lean_object* v_h__3_1142_, lean_object* v_h__4_1143_, lean_object* v_h__5_1144_, lean_object* v_h__6_1145_, lean_object* v_h__7_1146_){
_start:
{
switch(lean_obj_tag(v_x_1139_))
{
case 0:
{
lean_object* v_v_1147_; lean_object* v___x_1148_; 
lean_dec(v_h__6_1145_);
lean_dec(v_h__5_1144_);
lean_dec(v_h__4_1143_);
lean_dec(v_h__3_1142_);
lean_dec(v_h__2_1141_);
lean_dec(v_h__1_1140_);
v_v_1147_ = lean_ctor_get(v_x_1139_, 0);
lean_inc(v_v_1147_);
lean_dec_ref_known(v_x_1139_, 1);
v___x_1148_ = lean_apply_1(v_h__7_1146_, v_v_1147_);
return v___x_1148_;
}
case 1:
{
lean_object* v_f_1149_; lean_object* v_g_1150_; lean_object* v___x_1151_; 
lean_dec(v_h__7_1146_);
lean_dec(v_h__6_1145_);
lean_dec(v_h__5_1144_);
lean_dec(v_h__4_1143_);
lean_dec(v_h__3_1142_);
lean_dec(v_h__1_1140_);
v_f_1149_ = lean_ctor_get(v_x_1139_, 0);
lean_inc_ref(v_f_1149_);
v_g_1150_ = lean_ctor_get(v_x_1139_, 1);
lean_inc_ref(v_g_1150_);
lean_dec_ref_known(v_x_1139_, 2);
v___x_1151_ = lean_apply_2(v_h__2_1141_, v_f_1149_, v_g_1150_);
return v___x_1151_;
}
case 2:
{
lean_object* v_f_1152_; lean_object* v_g_1153_; lean_object* v___x_1154_; 
lean_dec(v_h__7_1146_);
lean_dec(v_h__6_1145_);
lean_dec(v_h__5_1144_);
lean_dec(v_h__4_1143_);
lean_dec(v_h__2_1141_);
lean_dec(v_h__1_1140_);
v_f_1152_ = lean_ctor_get(v_x_1139_, 0);
lean_inc_ref(v_f_1152_);
v_g_1153_ = lean_ctor_get(v_x_1139_, 1);
lean_inc_ref(v_g_1153_);
lean_dec_ref_known(v_x_1139_, 2);
v___x_1154_ = lean_apply_2(v_h__3_1142_, v_f_1152_, v_g_1153_);
return v___x_1154_;
}
case 3:
{
lean_object* v_f_1155_; lean_object* v_g_1156_; lean_object* v___x_1157_; 
lean_dec(v_h__7_1146_);
lean_dec(v_h__6_1145_);
lean_dec(v_h__5_1144_);
lean_dec(v_h__3_1142_);
lean_dec(v_h__2_1141_);
lean_dec(v_h__1_1140_);
v_f_1155_ = lean_ctor_get(v_x_1139_, 0);
lean_inc_ref(v_f_1155_);
v_g_1156_ = lean_ctor_get(v_x_1139_, 1);
lean_inc_ref(v_g_1156_);
lean_dec_ref_known(v_x_1139_, 2);
v___x_1157_ = lean_apply_2(v_h__4_1143_, v_f_1155_, v_g_1156_);
return v___x_1157_;
}
case 4:
{
lean_object* v_o_1158_; lean_object* v_k_1159_; lean_object* v___x_1160_; 
lean_dec(v_h__7_1146_);
lean_dec(v_h__6_1145_);
lean_dec(v_h__4_1143_);
lean_dec(v_h__3_1142_);
lean_dec(v_h__2_1141_);
lean_dec(v_h__1_1140_);
v_o_1158_ = lean_ctor_get(v_x_1139_, 0);
lean_inc(v_o_1158_);
v_k_1159_ = lean_ctor_get(v_x_1139_, 1);
lean_inc_ref(v_k_1159_);
lean_dec_ref_known(v_x_1139_, 2);
v___x_1160_ = lean_apply_2(v_h__5_1144_, v_o_1158_, v_k_1159_);
return v___x_1160_;
}
case 5:
{
lean_object* v_f_1161_; lean_object* v___x_1162_; 
lean_dec(v_h__7_1146_);
lean_dec(v_h__6_1145_);
lean_dec(v_h__5_1144_);
lean_dec(v_h__4_1143_);
lean_dec(v_h__3_1142_);
lean_dec(v_h__2_1141_);
v_f_1161_ = lean_ctor_get(v_x_1139_, 0);
lean_inc_ref(v_f_1161_);
lean_dec_ref_known(v_x_1139_, 1);
v___x_1162_ = lean_apply_1(v_h__1_1140_, v_f_1161_);
return v___x_1162_;
}
default: 
{
lean_object* v_f_1163_; lean_object* v___x_1164_; 
lean_dec(v_h__7_1146_);
lean_dec(v_h__5_1144_);
lean_dec(v_h__4_1143_);
lean_dec(v_h__3_1142_);
lean_dec(v_h__2_1141_);
lean_dec(v_h__1_1140_);
v_f_1163_ = lean_ctor_get(v_x_1139_, 0);
lean_inc_ref(v_f_1163_);
lean_dec_ref_known(v_x_1139_, 1);
v___x_1164_ = lean_apply_1(v_h__6_1145_, v_f_1163_);
return v___x_1164_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_StructuralInfo_0__StructuralInfo_containsOp_match__1_splitter___redArg(lean_object* v_x_1165_, lean_object* v_h__1_1166_, lean_object* v_h__2_1167_, lean_object* v_h__3_1168_, lean_object* v_h__4_1169_, lean_object* v_h__5_1170_, lean_object* v_h__6_1171_, lean_object* v_h__7_1172_){
_start:
{
switch(lean_obj_tag(v_x_1165_))
{
case 0:
{
lean_object* v_v_1173_; lean_object* v___x_1174_; 
lean_dec(v_h__6_1171_);
lean_dec(v_h__5_1170_);
lean_dec(v_h__4_1169_);
lean_dec(v_h__3_1168_);
lean_dec(v_h__2_1167_);
lean_dec(v_h__1_1166_);
v_v_1173_ = lean_ctor_get(v_x_1165_, 0);
lean_inc(v_v_1173_);
lean_dec_ref_known(v_x_1165_, 1);
v___x_1174_ = lean_apply_1(v_h__7_1172_, v_v_1173_);
return v___x_1174_;
}
case 1:
{
lean_object* v_f_1175_; lean_object* v_g_1176_; lean_object* v___x_1177_; 
lean_dec(v_h__7_1172_);
lean_dec(v_h__6_1171_);
lean_dec(v_h__5_1170_);
lean_dec(v_h__4_1169_);
lean_dec(v_h__3_1168_);
lean_dec(v_h__1_1166_);
v_f_1175_ = lean_ctor_get(v_x_1165_, 0);
lean_inc_ref(v_f_1175_);
v_g_1176_ = lean_ctor_get(v_x_1165_, 1);
lean_inc_ref(v_g_1176_);
lean_dec_ref_known(v_x_1165_, 2);
v___x_1177_ = lean_apply_2(v_h__2_1167_, v_f_1175_, v_g_1176_);
return v___x_1177_;
}
case 2:
{
lean_object* v_f_1178_; lean_object* v_g_1179_; lean_object* v___x_1180_; 
lean_dec(v_h__7_1172_);
lean_dec(v_h__6_1171_);
lean_dec(v_h__5_1170_);
lean_dec(v_h__4_1169_);
lean_dec(v_h__2_1167_);
lean_dec(v_h__1_1166_);
v_f_1178_ = lean_ctor_get(v_x_1165_, 0);
lean_inc_ref(v_f_1178_);
v_g_1179_ = lean_ctor_get(v_x_1165_, 1);
lean_inc_ref(v_g_1179_);
lean_dec_ref_known(v_x_1165_, 2);
v___x_1180_ = lean_apply_2(v_h__3_1168_, v_f_1178_, v_g_1179_);
return v___x_1180_;
}
case 3:
{
lean_object* v_f_1181_; lean_object* v_g_1182_; lean_object* v___x_1183_; 
lean_dec(v_h__7_1172_);
lean_dec(v_h__6_1171_);
lean_dec(v_h__5_1170_);
lean_dec(v_h__3_1168_);
lean_dec(v_h__2_1167_);
lean_dec(v_h__1_1166_);
v_f_1181_ = lean_ctor_get(v_x_1165_, 0);
lean_inc_ref(v_f_1181_);
v_g_1182_ = lean_ctor_get(v_x_1165_, 1);
lean_inc_ref(v_g_1182_);
lean_dec_ref_known(v_x_1165_, 2);
v___x_1183_ = lean_apply_2(v_h__4_1169_, v_f_1181_, v_g_1182_);
return v___x_1183_;
}
case 4:
{
lean_object* v_o_1184_; lean_object* v_k_1185_; lean_object* v___x_1186_; 
lean_dec(v_h__7_1172_);
lean_dec(v_h__6_1171_);
lean_dec(v_h__5_1170_);
lean_dec(v_h__4_1169_);
lean_dec(v_h__3_1168_);
lean_dec(v_h__2_1167_);
v_o_1184_ = lean_ctor_get(v_x_1165_, 0);
lean_inc(v_o_1184_);
v_k_1185_ = lean_ctor_get(v_x_1165_, 1);
lean_inc_ref(v_k_1185_);
lean_dec_ref_known(v_x_1165_, 2);
v___x_1186_ = lean_apply_2(v_h__1_1166_, v_o_1184_, v_k_1185_);
return v___x_1186_;
}
case 5:
{
lean_object* v_f_1187_; lean_object* v___x_1188_; 
lean_dec(v_h__7_1172_);
lean_dec(v_h__6_1171_);
lean_dec(v_h__4_1169_);
lean_dec(v_h__3_1168_);
lean_dec(v_h__2_1167_);
lean_dec(v_h__1_1166_);
v_f_1187_ = lean_ctor_get(v_x_1165_, 0);
lean_inc_ref(v_f_1187_);
lean_dec_ref_known(v_x_1165_, 1);
v___x_1188_ = lean_apply_1(v_h__5_1170_, v_f_1187_);
return v___x_1188_;
}
default: 
{
lean_object* v_f_1189_; lean_object* v___x_1190_; 
lean_dec(v_h__7_1172_);
lean_dec(v_h__5_1170_);
lean_dec(v_h__4_1169_);
lean_dec(v_h__3_1168_);
lean_dec(v_h__2_1167_);
lean_dec(v_h__1_1166_);
v_f_1189_ = lean_ctor_get(v_x_1165_, 0);
lean_inc_ref(v_f_1189_);
lean_dec_ref_known(v_x_1165_, 1);
v___x_1190_ = lean_apply_1(v_h__6_1171_, v_f_1189_);
return v___x_1190_;
}
}
}
}
LEAN_EXPORT lean_object* lp_arrow__minimality__lib___private_ArrowMinimalityLib_StructuralInfo_0__StructuralInfo_containsOp_match__1_splitter(lean_object* v_Op_1191_, lean_object* v_V_1192_, lean_object* v_motive_1193_, lean_object* v_x_1194_, lean_object* v_h__1_1195_, lean_object* v_h__2_1196_, lean_object* v_h__3_1197_, lean_object* v_h__4_1198_, lean_object* v_h__5_1199_, lean_object* v_h__6_1200_, lean_object* v_h__7_1201_){
_start:
{
switch(lean_obj_tag(v_x_1194_))
{
case 0:
{
lean_object* v_v_1202_; lean_object* v___x_1203_; 
lean_dec(v_h__6_1200_);
lean_dec(v_h__5_1199_);
lean_dec(v_h__4_1198_);
lean_dec(v_h__3_1197_);
lean_dec(v_h__2_1196_);
lean_dec(v_h__1_1195_);
v_v_1202_ = lean_ctor_get(v_x_1194_, 0);
lean_inc(v_v_1202_);
lean_dec_ref_known(v_x_1194_, 1);
v___x_1203_ = lean_apply_1(v_h__7_1201_, v_v_1202_);
return v___x_1203_;
}
case 1:
{
lean_object* v_f_1204_; lean_object* v_g_1205_; lean_object* v___x_1206_; 
lean_dec(v_h__7_1201_);
lean_dec(v_h__6_1200_);
lean_dec(v_h__5_1199_);
lean_dec(v_h__4_1198_);
lean_dec(v_h__3_1197_);
lean_dec(v_h__1_1195_);
v_f_1204_ = lean_ctor_get(v_x_1194_, 0);
lean_inc_ref(v_f_1204_);
v_g_1205_ = lean_ctor_get(v_x_1194_, 1);
lean_inc_ref(v_g_1205_);
lean_dec_ref_known(v_x_1194_, 2);
v___x_1206_ = lean_apply_2(v_h__2_1196_, v_f_1204_, v_g_1205_);
return v___x_1206_;
}
case 2:
{
lean_object* v_f_1207_; lean_object* v_g_1208_; lean_object* v___x_1209_; 
lean_dec(v_h__7_1201_);
lean_dec(v_h__6_1200_);
lean_dec(v_h__5_1199_);
lean_dec(v_h__4_1198_);
lean_dec(v_h__2_1196_);
lean_dec(v_h__1_1195_);
v_f_1207_ = lean_ctor_get(v_x_1194_, 0);
lean_inc_ref(v_f_1207_);
v_g_1208_ = lean_ctor_get(v_x_1194_, 1);
lean_inc_ref(v_g_1208_);
lean_dec_ref_known(v_x_1194_, 2);
v___x_1209_ = lean_apply_2(v_h__3_1197_, v_f_1207_, v_g_1208_);
return v___x_1209_;
}
case 3:
{
lean_object* v_f_1210_; lean_object* v_g_1211_; lean_object* v___x_1212_; 
lean_dec(v_h__7_1201_);
lean_dec(v_h__6_1200_);
lean_dec(v_h__5_1199_);
lean_dec(v_h__3_1197_);
lean_dec(v_h__2_1196_);
lean_dec(v_h__1_1195_);
v_f_1210_ = lean_ctor_get(v_x_1194_, 0);
lean_inc_ref(v_f_1210_);
v_g_1211_ = lean_ctor_get(v_x_1194_, 1);
lean_inc_ref(v_g_1211_);
lean_dec_ref_known(v_x_1194_, 2);
v___x_1212_ = lean_apply_2(v_h__4_1198_, v_f_1210_, v_g_1211_);
return v___x_1212_;
}
case 4:
{
lean_object* v_o_1213_; lean_object* v_k_1214_; lean_object* v___x_1215_; 
lean_dec(v_h__7_1201_);
lean_dec(v_h__6_1200_);
lean_dec(v_h__5_1199_);
lean_dec(v_h__4_1198_);
lean_dec(v_h__3_1197_);
lean_dec(v_h__2_1196_);
v_o_1213_ = lean_ctor_get(v_x_1194_, 0);
lean_inc(v_o_1213_);
v_k_1214_ = lean_ctor_get(v_x_1194_, 1);
lean_inc_ref(v_k_1214_);
lean_dec_ref_known(v_x_1194_, 2);
v___x_1215_ = lean_apply_2(v_h__1_1195_, v_o_1213_, v_k_1214_);
return v___x_1215_;
}
case 5:
{
lean_object* v_f_1216_; lean_object* v___x_1217_; 
lean_dec(v_h__7_1201_);
lean_dec(v_h__6_1200_);
lean_dec(v_h__4_1198_);
lean_dec(v_h__3_1197_);
lean_dec(v_h__2_1196_);
lean_dec(v_h__1_1195_);
v_f_1216_ = lean_ctor_get(v_x_1194_, 0);
lean_inc_ref(v_f_1216_);
lean_dec_ref_known(v_x_1194_, 1);
v___x_1217_ = lean_apply_1(v_h__5_1199_, v_f_1216_);
return v___x_1217_;
}
default: 
{
lean_object* v_f_1218_; lean_object* v___x_1219_; 
lean_dec(v_h__7_1201_);
lean_dec(v_h__5_1199_);
lean_dec(v_h__4_1198_);
lean_dec(v_h__3_1197_);
lean_dec(v_h__2_1196_);
lean_dec(v_h__1_1195_);
v_f_1218_ = lean_ctor_get(v_x_1194_, 0);
lean_inc_ref(v_f_1218_);
lean_dec_ref_known(v_x_1194_, 1);
v___x_1219_ = lean_apply_1(v_h__6_1200_, v_f_1218_);
return v___x_1219_;
}
}
}
}
lean_object* initialize_Init(uint8_t builtin);
lean_object* initialize_Init(uint8_t builtin);
void lean_initialize_runtime_module();
static bool _G_initialized = false;
LEAN_EXPORT lean_object* initialize_arrow__minimality__lib_ArrowMinimalityLib_StructuralInfo(uint8_t builtin) {
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
