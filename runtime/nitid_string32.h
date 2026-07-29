#ifndef NITID_STRING32_H
#define NITID_STRING32_H

#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
#include "nitid_string.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    uint32_t *data;
    size_t len;
    size_t cap;
} nitid_string32;

nitid_string32 nitid_string32_from_utf8(const char *utf8);
nitid_string32 nitid_string32_from_n(const uint32_t *s, size_t n);
nitid_string32 nitid_string32_from_utf16(const uint16_t *s, size_t n);
nitid_string32 nitid_string32_clone(const nitid_string32 *s);
void nitid_string32_free(nitid_string32 *s);
uint32_t nitid_string32_at(const nitid_string32 *s, size_t index);
size_t nitid_string32_len(const nitid_string32 *s);
nitid_string nitid_string32_to_utf8(const nitid_string32 *s);
uint16_t* nitid_string32_to_utf16(const nitid_string32 *s, size_t *out_len);

uint32_t nitid_string32_at_cp(const nitid_string32 *s, int64_t index);
nitid_string32 nitid_string32_concat(const nitid_string32 *a, const nitid_string32 *b);
bool nitid_string32_eq(const nitid_string32 *a, const nitid_string32 *b);
bool nitid_string32_ne(const nitid_string32 *a, const nitid_string32 *b);
bool nitid_string32_lt(const nitid_string32 *a, const nitid_string32 *b);
bool nitid_string32_le(const nitid_string32 *a, const nitid_string32 *b);
bool nitid_string32_gt(const nitid_string32 *a, const nitid_string32 *b);
bool nitid_string32_ge(const nitid_string32 *a, const nitid_string32 *b);

#ifdef __cplusplus
}
#endif

#endif /* NITID_STRING32_H */
