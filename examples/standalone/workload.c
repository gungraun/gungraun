#include <errno.h>
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

/* Keep a named boundary for Callgrind even in an optimized build. */
#if defined(__GNUC__) || defined(__clang__)
__attribute__((noinline))
#endif
uint64_t
gungraun_example_region(const uint64_t* input, size_t count)
{
  volatile uint64_t sum = 0;
  for (size_t index = 0; index < count; ++index) {
    sum += input[index];
  }
  return sum;
}

int
main(int argc, char** argv)
{
  if (argc != 2) {
    fputs("usage: workload COUNT (1..4096)\n", stderr);
    return 1;
  }

  char* end = NULL;
  errno = 0;
  unsigned long parsed = strtoul(argv[1], &end, 10);
  if (errno != 0 || end == argv[1] || *end != '\0' || parsed == 0 ||
      parsed > 4096) {
    fputs("COUNT must be an integer between 1 and 4096\n", stderr);
    return 1;
  }

  /* Build the fixture before entering the measured region. */
  size_t count = (size_t)parsed;
  uint64_t input[4096];
  for (size_t index = 0; index < count; ++index) {
    input[index] = (uint64_t)index + 1;
  }

  uint64_t actual = gungraun_example_region(input, count);

  /* Check correctness afterwards, including in builds with NDEBUG defined. */
  uint64_t expected = (uint64_t)count * ((uint64_t)count + 1) / 2;
  if (actual != expected) {
    fprintf(stderr, "incorrect sum: %" PRIu64 " != %" PRIu64 "\n", actual,
            expected);
    return 2;
  }
  printf("count=%zu sum=%" PRIu64 "\n", count, actual);
  return 0;
}
