/* sdkgen spec for libcurl — NOT a vendored header.
 *
 * `hnl sdkgen c` only collects functions declared in the main file, but
 * the easy API lives in <curl/easy.h> (reached via curl.h's include
 * closure). This spec includes the real <curl/curl.h> for types and
 * constants, then re-declares the functions we bind so they are
 * main-file decls. Identical signatures, legal C.
 *
 * Regenerate:
 *   hnl sdkgen c handl-packages/generated/curl-spec/curl.h \
 *     --output handl-packages/generated/curl \
 *     --config handl-packages/generated/curl.sdkgen.yaml
 *
 * The variadic functions (curl_easy_setopt, curl_easy_getinfo) are not
 * bound here — fetch-c.hnl calls them through `ffi.call` directly, like
 * libc's `syscall`.
 */
#include <curl/curl.h>

CURL *curl_easy_init(void);
CURLcode curl_easy_perform(CURL *curl);
void curl_easy_cleanup(CURL *curl);
const char *curl_easy_strerror(CURLcode errornum);
struct curl_slist *curl_slist_append(struct curl_slist *list,
                                     const char *string);
void curl_slist_free_all(struct curl_slist *list);
