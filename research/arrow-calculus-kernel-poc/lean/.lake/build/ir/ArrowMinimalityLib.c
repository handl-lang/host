// Lean compiler output
// Module: ArrowMinimalityLib
// Imports: public import Init public meta import Init public import ArrowMinimalityLib.Basic public import ArrowMinimalityLib.ProofGraphs public import ArrowMinimalityLib.IteratedLoop public import ArrowMinimalityLib.Translation public import ArrowMinimalityLib.DependentProofGraphs public import ArrowMinimalityLib.RefinementProofGraphs public import ArrowMinimalityLib.ProofAdvantage public import ArrowMinimalityLib.ProofRelevant public import ArrowMinimalityLib.FanoutUnencodable public import ArrowMinimalityLib.RefinementCharacterization public import ArrowMinimalityLib.InvariantDomain public import ArrowMinimalityLib.StructuralInference public import ArrowMinimalityLib.LoopInvariantDiscovery public import ArrowMinimalityLib.AlgorithmicCompleteness public import ArrowMinimalityLib.Termination public import ArrowMinimalityLib.TraitInference
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
lean_object* initialize_Init(uint8_t builtin);
lean_object* initialize_Init(uint8_t builtin);
lean_object* initialize_arrow__minimality__lib_ArrowMinimalityLib_Basic(uint8_t builtin);
lean_object* initialize_arrow__minimality__lib_ArrowMinimalityLib_ProofGraphs(uint8_t builtin);
lean_object* initialize_arrow__minimality__lib_ArrowMinimalityLib_IteratedLoop(uint8_t builtin);
lean_object* initialize_arrow__minimality__lib_ArrowMinimalityLib_Translation(uint8_t builtin);
lean_object* initialize_arrow__minimality__lib_ArrowMinimalityLib_DependentProofGraphs(uint8_t builtin);
lean_object* initialize_arrow__minimality__lib_ArrowMinimalityLib_RefinementProofGraphs(uint8_t builtin);
lean_object* initialize_arrow__minimality__lib_ArrowMinimalityLib_ProofAdvantage(uint8_t builtin);
lean_object* initialize_arrow__minimality__lib_ArrowMinimalityLib_ProofRelevant(uint8_t builtin);
lean_object* initialize_arrow__minimality__lib_ArrowMinimalityLib_FanoutUnencodable(uint8_t builtin);
lean_object* initialize_arrow__minimality__lib_ArrowMinimalityLib_RefinementCharacterization(uint8_t builtin);
lean_object* initialize_arrow__minimality__lib_ArrowMinimalityLib_InvariantDomain(uint8_t builtin);
lean_object* initialize_arrow__minimality__lib_ArrowMinimalityLib_StructuralInference(uint8_t builtin);
lean_object* initialize_arrow__minimality__lib_ArrowMinimalityLib_LoopInvariantDiscovery(uint8_t builtin);
lean_object* initialize_arrow__minimality__lib_ArrowMinimalityLib_AlgorithmicCompleteness(uint8_t builtin);
lean_object* initialize_arrow__minimality__lib_ArrowMinimalityLib_Termination(uint8_t builtin);
lean_object* initialize_arrow__minimality__lib_ArrowMinimalityLib_TraitInference(uint8_t builtin);
static bool _G_initialized = false;
LEAN_EXPORT lean_object* initialize_arrow__minimality__lib_ArrowMinimalityLib(uint8_t builtin) {
lean_object * res;
if (_G_initialized) return lean_io_result_mk_ok(lean_box(0));
_G_initialized = true;
res = initialize_Init(builtin);
if (lean_io_result_is_error(res)) return res;
lean_dec_ref(res);
res = initialize_Init(builtin);
if (lean_io_result_is_error(res)) return res;
lean_dec_ref(res);
res = initialize_arrow__minimality__lib_ArrowMinimalityLib_Basic(builtin);
if (lean_io_result_is_error(res)) return res;
lean_dec_ref(res);
res = initialize_arrow__minimality__lib_ArrowMinimalityLib_ProofGraphs(builtin);
if (lean_io_result_is_error(res)) return res;
lean_dec_ref(res);
res = initialize_arrow__minimality__lib_ArrowMinimalityLib_IteratedLoop(builtin);
if (lean_io_result_is_error(res)) return res;
lean_dec_ref(res);
res = initialize_arrow__minimality__lib_ArrowMinimalityLib_Translation(builtin);
if (lean_io_result_is_error(res)) return res;
lean_dec_ref(res);
res = initialize_arrow__minimality__lib_ArrowMinimalityLib_DependentProofGraphs(builtin);
if (lean_io_result_is_error(res)) return res;
lean_dec_ref(res);
res = initialize_arrow__minimality__lib_ArrowMinimalityLib_RefinementProofGraphs(builtin);
if (lean_io_result_is_error(res)) return res;
lean_dec_ref(res);
res = initialize_arrow__minimality__lib_ArrowMinimalityLib_ProofAdvantage(builtin);
if (lean_io_result_is_error(res)) return res;
lean_dec_ref(res);
res = initialize_arrow__minimality__lib_ArrowMinimalityLib_ProofRelevant(builtin);
if (lean_io_result_is_error(res)) return res;
lean_dec_ref(res);
res = initialize_arrow__minimality__lib_ArrowMinimalityLib_FanoutUnencodable(builtin);
if (lean_io_result_is_error(res)) return res;
lean_dec_ref(res);
res = initialize_arrow__minimality__lib_ArrowMinimalityLib_RefinementCharacterization(builtin);
if (lean_io_result_is_error(res)) return res;
lean_dec_ref(res);
res = initialize_arrow__minimality__lib_ArrowMinimalityLib_InvariantDomain(builtin);
if (lean_io_result_is_error(res)) return res;
lean_dec_ref(res);
res = initialize_arrow__minimality__lib_ArrowMinimalityLib_StructuralInference(builtin);
if (lean_io_result_is_error(res)) return res;
lean_dec_ref(res);
res = initialize_arrow__minimality__lib_ArrowMinimalityLib_LoopInvariantDiscovery(builtin);
if (lean_io_result_is_error(res)) return res;
lean_dec_ref(res);
res = initialize_arrow__minimality__lib_ArrowMinimalityLib_AlgorithmicCompleteness(builtin);
if (lean_io_result_is_error(res)) return res;
lean_dec_ref(res);
res = initialize_arrow__minimality__lib_ArrowMinimalityLib_Termination(builtin);
if (lean_io_result_is_error(res)) return res;
lean_dec_ref(res);
res = initialize_arrow__minimality__lib_ArrowMinimalityLib_TraitInference(builtin);
if (lean_io_result_is_error(res)) return res;
lean_dec_ref(res);
return lean_io_result_mk_ok(lean_box(0));
}
#ifdef __cplusplus
}
#endif
