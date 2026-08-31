#ifndef NITID_STRING_H
#define NITID_STRING_H

#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    char *data;
    size_t len;
    size_t cap;
} nitid_string;

nitid_string nitid_string_from(const char *s);
nitid_string nitid_string_from_n(const char *s, size_t n);
nitid_string nitid_string_clone(const nitid_string *s);
void nitid_string_free(nitid_string *s);
char nitid_string_at(const nitid_string *s, size_t index);
void nitid_string_append(nitid_string *s, const char *data, size_t n);

uint32_t nitid_string_at_cp(const nitid_string *s, int64_t index);
nitid_string nitid_string_concat(const nitid_string *a, const nitid_string *b);
bool nitid_string_eq(const nitid_string *a, const nitid_string *b);
bool nitid_string_ne(const nitid_string *a, const nitid_string *b);
bool nitid_string_lt(const nitid_string *a, const nitid_string *b);
bool nitid_string_le(const nitid_string *a, const nitid_string *b);
bool nitid_string_gt(const nitid_string *a, const nitid_string *b);
bool nitid_string_ge(const nitid_string *a, const nitid_string *b);

#ifdef __cplusplus
}
#endif

#endif /* NITID_STRING_H */
