#include <stddef.h>

#include "valgrind/valgrind.h"

size_t valgrind_do_client_request_expr(size_t _zzq_default, size_t _zzq_request,
                                       size_t _zzq_arg1, size_t _zzq_arg2,
                                       size_t _zzq_arg3, size_t _zzq_arg4,
                                       size_t _zzq_arg5) {
#ifdef VALGRIND_DO_CLIENT_REQUEST_EXPR
  return VALGRIND_DO_CLIENT_REQUEST_EXPR(_zzq_default, _zzq_request, _zzq_arg1,
                                         _zzq_arg2, _zzq_arg3, _zzq_arg4,
                                         _zzq_arg5);
#else
  (void)_zzq_request;
  (void)_zzq_arg1;
  (void)_zzq_arg2;
  (void)_zzq_arg3;
  (void)_zzq_arg4;
  (void)_zzq_arg5;

  return _zzq_default;
#endif
}

int valgrind_printf(char *message) { return VALGRIND_PRINTF("%s", message); }

int valgrind_printf_backtrace(char *message) {
  return VALGRIND_PRINTF_BACKTRACE("%s", message);
}
