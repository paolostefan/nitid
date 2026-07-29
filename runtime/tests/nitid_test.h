#ifndef NITID_TEST_H
#define NITID_TEST_H

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern int _test_passed;
extern int _test_failed;

#define TEST_GLOBALS \
int _test_passed = 0; \
int _test_failed = 0;

#define TEST(name) static void test_##name(void)
#define ASSERT(cond) do { \
    if (!(cond)) { \
        fprintf(stderr, "  FAIL at %s:%d: %s\n", __FILE__, __LINE__, #cond); \
        _test_failed++; \
        return; \
    } \
} while (0)
#define ASSERT_EQ(a, b) do { \
    if ((a) != (b)) { \
        fprintf(stderr, "  FAIL at %s:%d: expected %ld, got %ld\n", __FILE__, __LINE__, (long)(b), (long)(a)); \
        _test_failed++; \
        return; \
    } \
} while (0)
#define ASSERT_STR_EQ(a, b) do { \
    if (strcmp((a), (b)) != 0) { \
        fprintf(stderr, "  FAIL at %s:%d: expected \"%s\", got \"%s\"\n", __FILE__, __LINE__, (b), (a)); \
        _test_failed++; \
        return; \
    } \
} while (0)
#define ASSERT_NULL(p) do { \
    if ((p) != NULL) { \
        fprintf(stderr, "  FAIL at %s:%d: expected NULL\n", __FILE__, __LINE__); \
        _test_failed++; \
        return; \
    } \
} while (0)
#define ASSERT_NOT_NULL(p) do { \
    if ((p) == NULL) { \
        fprintf(stderr, "  FAIL at %s:%d: expected non-NULL\n", __FILE__, __LINE__); \
        _test_failed++; \
        return; \
    } \
} while (0)

#define RUN_TEST(name) do { \
    printf("  %-30s ", #name); \
    fflush(stdout); \
    int before = _test_failed; \
    test_##name(); \
    if (_test_failed == before) { \
        printf("PASS\n"); \
        _test_passed++; \
    } else { \
        printf("FAIL\n"); \
    } \
} while (0)

static int test_summary(void) {
    printf("\n%d passed, %d failed\n", _test_passed, _test_failed);
    return _test_failed > 0 ? 1 : 0;
}

#endif /* NITID_TEST_H */
