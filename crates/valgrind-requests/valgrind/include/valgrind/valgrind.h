#ifndef __VALGRIND_H
#define __VALGRIND_H

static int VALGRIND_PRINTF(const char *format, ...) {
  (void)format;
  return 0;
}

static int VALGRIND_PRINTF_BACKTRACE(const char *format, ...) {
  (void)format;
  return 0;
}

#endif
