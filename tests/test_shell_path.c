#ifndef __KERNEL_HOST_TEST__
#define __KERNEL_HOST_TEST__
#endif

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

#include "shell.h"

static int tests_run = 0;
static int tests_passed = 0;

#define RUN_TEST(fn) do { \
    tests_run++; \
    printf("[TEST %2d] Running %-50s ... ", tests_run, #fn); \
    fflush(stdout); \
    if (fn()) { \
        tests_passed++; \
        printf("PASSED\n"); \
    } else { \
        printf("FAILED\n"); \
    } \
    fflush(stdout); \
} while(0)

/* 1. A plain filename remains unchanged */
static int test_plain_filename(void) {
    char out[32];
    memset(out, 0, sizeof(out));
    if (shell_normalize_path("welcome.txt", out, sizeof(out)) != 0) return 0;
    if (strcmp(out, "welcome.txt") != 0) return 0;

    memset(out, 0, sizeof(out));
    if (shell_normalize_path("a", out, sizeof(out)) != 0) return 0;
    if (strcmp(out, "a") != 0) return 0;

    memset(out, 0, sizeof(out));
    if (shell_normalize_path("kernel.elf", out, sizeof(out)) != 0) return 0;
    if (strcmp(out, "kernel.elf") != 0) return 0;

    return 1;
}

/* 2. ./welcome.txt normalizes to welcome.txt */
static int test_dot_slash_prefix(void) {
    char out[32];
    memset(out, 0, sizeof(out));
    if (shell_normalize_path("./welcome.txt", out, sizeof(out)) != 0) return 0;
    if (strcmp(out, "welcome.txt") != 0) return 0;

    memset(out, 0, sizeof(out));
    if (shell_normalize_path("./data.bin", out, sizeof(out)) != 0) return 0;
    if (strcmp(out, "data.bin") != 0) return 0;

    return 1;
}

/* 3. /welcome.txt normalizes to welcome.txt */
static int test_leading_slash_prefix(void) {
    char out[32];
    memset(out, 0, sizeof(out));
    if (shell_normalize_path("/welcome.txt", out, sizeof(out)) != 0) return 0;
    if (strcmp(out, "welcome.txt") != 0) return 0;

    memset(out, 0, sizeof(out));
    if (shell_normalize_path("/version.txt", out, sizeof(out)) != 0) return 0;
    if (strcmp(out, "version.txt") != 0) return 0;

    return 1;
}

/* 4. Multiple supported leading prefixes behave as specified */
static int test_multiple_supported_prefixes(void) {
    char out[32];

    memset(out, 0, sizeof(out));
    if (shell_normalize_path("//welcome.txt", out, sizeof(out)) != 0) return 0;
    if (strcmp(out, "welcome.txt") != 0) return 0;

    memset(out, 0, sizeof(out));
    if (shell_normalize_path(".//welcome.txt", out, sizeof(out)) != 0) return 0;
    if (strcmp(out, "welcome.txt") != 0) return 0;

    memset(out, 0, sizeof(out));
    if (shell_normalize_path("././welcome.txt", out, sizeof(out)) != 0) return 0;
    if (strcmp(out, "welcome.txt") != 0) return 0;

    memset(out, 0, sizeof(out));
    if (shell_normalize_path("/./welcome.txt", out, sizeof(out)) != 0) return 0;
    if (strcmp(out, "welcome.txt") != 0) return 0;

    memset(out, 0, sizeof(out));
    if (shell_normalize_path("///note.txt", out, sizeof(out)) != 0) return 0;
    if (strcmp(out, "note.txt") != 0) return 0;

    memset(out, 0, sizeof(out));
    if (shell_normalize_path("./././test.dat", out, sizeof(out)) != 0) return 0;
    if (strcmp(out, "test.dat") != 0) return 0;

    return 1;
}

/* 5. An empty path is rejected safely */
static int test_empty_paths_rejected(void) {
    char out[32];
    if (shell_normalize_path("", out, sizeof(out)) == 0) return 0;
    if (shell_normalize_path(NULL, out, sizeof(out)) == 0) return 0;
    if (shell_normalize_path("./", out, sizeof(out)) == 0) return 0;
    if (shell_normalize_path("././", out, sizeof(out)) == 0) return 0;
    return 1;
}

/* 6. / is rejected as a file path */
static int test_slash_only_rejected(void) {
    char out[32];
    if (shell_normalize_path("/", out, sizeof(out)) == 0) return 0;
    if (shell_normalize_path("//", out, sizeof(out)) == 0) return 0;
    if (shell_normalize_path("///", out, sizeof(out)) == 0) return 0;
    return 1;
}

/* 7. A path containing unsupported nested components is not misinterpreted as root-level file */
static int test_unsupported_nested_components(void) {
    char out[32];
    if (shell_normalize_path("folder/file.txt", out, sizeof(out)) == 0) return 0;
    if (shell_normalize_path("/folder/file.txt", out, sizeof(out)) == 0) return 0;
    if (shell_normalize_path("./folder/file.txt", out, sizeof(out)) == 0) return 0;
    if (shell_normalize_path("a/b/c", out, sizeof(out)) == 0) return 0;
    if (shell_normalize_path("/a/b", out, sizeof(out)) == 0) return 0;
    if (shell_normalize_path("dir/", out, sizeof(out)) == 0) return 0;
    if (shell_normalize_path("./dir/", out, sizeof(out)) == 0) return 0;
    return 1;
}

/* 8. A filename at the actual maximum supported length (23 chars) is handled correctly */
static int test_max_filename_length(void) {
    char out[32];
    const char *exact_max = "12345678901234567890123"; /* Exactly 23 chars */
    assert(strlen(exact_max) == 23);

    memset(out, 0, sizeof(out));
    if (shell_normalize_path(exact_max, out, sizeof(out)) != 0) return 0;
    if (strcmp(out, exact_max) != 0) return 0;

    memset(out, 0, sizeof(out));
    if (shell_normalize_path("./12345678901234567890123", out, sizeof(out)) != 0) return 0;
    if (strcmp(out, exact_max) != 0) return 0;

    memset(out, 0, sizeof(out));
    if (shell_normalize_path("/12345678901234567890123", out, sizeof(out)) != 0) return 0;
    if (strcmp(out, exact_max) != 0) return 0;

    return 1;
}

/* 9. An overlong filename is rejected without truncation */
static int test_overlong_filename_rejected_without_truncation(void) {
    char out[32];
    memset(out, 0xEE, sizeof(out));

    const char *too_long_24 = "123456789012345678901234"; /* 24 chars */
    assert(strlen(too_long_24) == 24);
    if (shell_normalize_path(too_long_24, out, sizeof(out)) == 0) return 0;

    const char *too_long_30 = "very_long_file_name_exceeding_capacity.txt";
    if (shell_normalize_path(too_long_30, out, sizeof(out)) == 0) return 0;

    if (shell_normalize_path("/123456789012345678901234", out, sizeof(out)) == 0) return 0;
    if (shell_normalize_path("./123456789012345678901234", out, sizeof(out)) == 0) return 0;

    return 1;
}

/* 10. A small buffer or boundary case cannot cause an out-of-bounds access */
static int test_buffer_capacity_and_boundary_safety(void) {
    char guard_before[8];
    char out[12];
    char guard_after[8];

    memset(guard_before, 0xAA, sizeof(guard_before));
    memset(guard_after, 0x55, sizeof(guard_after));

    /* NULL output or zero capacity */
    if (shell_normalize_path("welcome.txt", NULL, 24) == 0) return 0;
    if (shell_normalize_path("welcome.txt", out, 0) == 0) return 0;

    /* Out capacity too small: "welcome.txt" is 11 chars + 1 null = 12 bytes needed */
    /* Capacity 5: must fail without writing past 5 */
    if (shell_normalize_path("welcome.txt", out, 5) == 0) return 0;

    /* Capacity 11: 11 chars fits, but no room for null terminator! Must reject. */
    if (shell_normalize_path("welcome.txt", out, 11) == 0) return 0;

    /* Capacity 12: exact fit for 11 chars + null terminator */
    if (shell_normalize_path("welcome.txt", out, 12) != 0) return 0;
    if (strcmp(out, "welcome.txt") != 0) return 0;

    /* Verify guards were never modified */
    for (size_t i = 0; i < sizeof(guard_before); i++) {
        if ((unsigned char)guard_before[i] != 0xAA) return 0;
    }
    for (size_t i = 0; i < sizeof(guard_after); i++) {
        if ((unsigned char)guard_after[i] != 0x55) return 0;
    }

    return 1;
}

int main(void) {
    printf("=====================================================\n");
    printf(" Running Shell Path Normalization Unit Tests (10 Cases)\n");
    printf("=====================================================\n");

    RUN_TEST(test_plain_filename);
    RUN_TEST(test_dot_slash_prefix);
    RUN_TEST(test_leading_slash_prefix);
    RUN_TEST(test_multiple_supported_prefixes);
    RUN_TEST(test_empty_paths_rejected);
    RUN_TEST(test_slash_only_rejected);
    RUN_TEST(test_unsupported_nested_components);
    RUN_TEST(test_max_filename_length);
    RUN_TEST(test_overlong_filename_rejected_without_truncation);
    RUN_TEST(test_buffer_capacity_and_boundary_safety);

    printf("=====================================================\n");
    printf(" Shell Path Normalization Tests: %d/%d passed\n", tests_passed, tests_run);
    printf("=====================================================\n");

    return (tests_passed == tests_run) ? 0 : 1;
}
