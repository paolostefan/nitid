#ifndef NITID_STRING16_H
#define NITID_STRING16_H

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
    uint16_t *data;
    size_t len;
    size_t cap;
} nitid_string16;

nitid_string16 nitid_string16_from_utf8(const char *utf8);
nitid_string16 nitid_string16_from_n(const uint16_t *s, size_t n);
nitid_string16 nitid_string16_from_utf32(const uint32_t *s, size_t n);
nitid_string16 nitid_string16_clone(const nitid_string16 *s);
void nitid_string16_free(nitid_string16 *s);
uint32_t nitid_string16_at(const nitid_string16 *s, size_t index);
size_t nitid_string16_len(const nitid_string16 *s);
nitid_string nitid_string16_to_utf8(const nitid_string16 *s);
uint32_t* nitid_string16_to_utf32(const nitid_string16 *s, size_t *out_len);

uint32_t nitid_string16_at_cp(const nitid_string16 *s, int64_t index);
nitid_string16 nitid_string16_concat(const nitid_string16 *a, const nitid_string16 *b);
bool nitid_string16_eq(const nitid_string16 *a, const nitid_string16 *b);
bool nitid_string16_ne(const nitid_string16 *a, const nitid_string16 *b);
bool nitid_string16_lt(const nitid_string16 *a, const nitid_string16 *b);
bool nitid_string16_le(const nitid_string16 *a, const nitid_string16 *b);
bool nitid_string16_gt(const nitid_string16 *a, const nitid_string16 *b);
bool nitid_string16_ge(const nitid_string16 *a, const nitid_string16 *b);

#ifdef __cplusplus
}
#endif

#endif /* NITID_STRING16_H */
