#include <assert.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

/* The fixture deliberately keeps setup and correctness checks outside this symbol. */
__attribute__((noinline)) uint64_t guard_region(unsigned count) {
    volatile uint64_t value = 0;
    for (unsigned i = 0; i < count; ++i) {
        value += 3 * i + 1;
    }
    return value;
}

static void check_context(int argc, char **argv) {
    assert(argc == 6);
    assert(strcmp(argv[2], "argument with spaces") == 0);
    assert(strcmp(argv[3], "${LITERAL}") == 0);
    assert(strcmp(argv[4], "$(touch SH_INJECTION)") == 0);
    assert(strcmp(argv[5], "*") == 0);
    assert(getenv("DEFAULT_ENV") != NULL);
    assert(strcmp(getenv("DEFAULT_ENV"), "default value") == 0);
    assert(getenv("OVERRIDE") != NULL);
    assert(strcmp(getenv("OVERRIDE"), "case value") == 0);
    assert(getenv("LITERAL") != NULL);
    assert(strcmp(getenv("LITERAL"), "$HOME ${USER} *") == 0);
    FILE *marker = fopen("marker", "r");
    assert(marker != NULL);
    assert(fclose(marker) == 0);

    char cwd[4096];
    assert(getcwd(cwd, sizeof(cwd)) != NULL);
    FILE *observed = fopen("observed.txt", "w");
    assert(observed != NULL);
    for (int i = 0; i < argc; ++i) {
        fprintf(observed, "argv[%d]=%s\n", i, argv[i]);
    }
    fprintf(observed, "cwd=%s\nDEFAULT_ENV=%s\nOVERRIDE=%s\nLITERAL=%s\n",
            cwd, getenv("DEFAULT_ENV"), getenv("OVERRIDE"), getenv("LITERAL"));
    assert(fclose(observed) == 0);
}

/* Used only by the malformed-profiler-output test; it is not a measurement source. */
static void corrupt_output(const char *pattern) {
    char path[4096];
    const char *pid = strstr(pattern, "%p");
    if (pid != NULL) {
        int prefix = (int)(pid - pattern);
        assert(snprintf(path, sizeof(path), "%.*s%ld%s", prefix, pattern,
                        (long)getpid(), pid + 2) > 0);
    } else {
        assert(snprintf(path, sizeof(path), "%s", pattern) > 0);
    }
    FILE *file = fopen(path, "w");
    assert(file != NULL);
    fputs("This is deliberately invalid Callgrind output.\n", file);
    assert(fclose(file) == 0);
}

int main(int argc, char **argv) {
    int fake_tool = 0;
    for (int i = 1; i < argc; ++i) {
        const char *output = "--callgrind-out-file=";
        const char *log = "--log-file=";
        if (strncmp(argv[i], output, strlen(output)) == 0) {
            corrupt_output(argv[i] + strlen(output));
            fake_tool = 1;
        } else if (strncmp(argv[i], log, strlen(log)) == 0) {
            corrupt_output(argv[i] + strlen(log));
        }
    }
    if (fake_tool) {
        return 0;
    }

    assert(argc >= 2);
    const char *mode = argv[1];
    if (strcmp(mode, "skip") == 0) {
        return 0;
    }
    if (strcmp(mode, "context") == 0) {
        check_context(argc, argv);
    }
    uint64_t result = guard_region(32);
    assert(result == 1520);
    if (strcmp(mode, "fail") == 0) {
        return 9;
    }
    if (strcmp(mode, "assert") == 0) {
        assert(result == 0);
    }
    if (strcmp(mode, "signal") == 0) {
        raise(SIGTERM);
    }
    return strcmp(mode, "exit-seven") == 0 ? 7 : 0;
}
