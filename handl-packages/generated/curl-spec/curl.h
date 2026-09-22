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
#include <curl/multi.h>

CURL *curl_easy_init(void);
CURLcode curl_easy_perform(CURL *curl);
void curl_easy_cleanup(CURL *curl);
const char *curl_easy_strerror(CURLcode errornum);
struct curl_slist *curl_slist_append(struct curl_slist *list,
                                     const char *string);
void curl_slist_free_all(struct curl_slist *list);

/* multi API — driven by concurrent-epoll via fdset->epoll sync (no
 * socket callbacks: FFI cannot marshal C->handl trampolines). */
CURLM *curl_multi_init(void);
CURLMcode curl_multi_add_handle(CURLM *multi, CURL *easy);
CURLMcode curl_multi_remove_handle(CURLM *multi, CURL *easy);
CURLMcode curl_multi_perform(CURLM *multi, int *running_handles);
CURLMcode curl_multi_fdset(CURLM *multi, fd_set *read_fd_set,
                           fd_set *write_fd_set, fd_set *exc_fd_set,
                           int *max_fd);
CURLMcode curl_multi_timeout(CURLM *multi, long *timeout_ms);
CURLMsg *curl_multi_info_read(CURLM *multi, int *msgs_in_queue);
CURLMcode curl_multi_cleanup(CURLM *multi);
const char *curl_multi_strerror(CURLMcode code);
