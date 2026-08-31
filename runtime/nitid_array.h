#ifndef NITID_ARRAY_H
#define NITID_ARRAY_H

#include <stddef.h>
#include <stdint.h>
#include <stdbool.h>

#include "nitid_types.h"

typedef struct {
  size_t length;    // Number of current array members
  size_t capacity;  // Maximum number of array members
  size_t elem_size; // sizeof() each array member
  void *data;       // Memory of the array
} nitid_array;

#define NITID_ARRAY_FROM_LIT(typ) \
nitid_array nitid_array_from_lit_##typ(size_t count, const typ values[])

#define NITID_ARRAY_FROM_LIT_IMPL(typ) \
  nitid_array nitid_array_from_lit_##typ(const size_t count, const typ values[]) {\
  nitid_array arr;\
  arr.data = malloc(count * sizeof(typ));\
  arr.length = count;\
  arr.capacity = count;\
  arr.elem_size = sizeof(typ);\
  memcpy(arr.data, values, count * sizeof(typ));\
  return arr;\
}

#define NITID_ARRAY_GET(typ) typ nitid_array_get_##typ(const nitid_array arr, int64_t index)
#define NITID_ARRAY_GET_IMPL(typ) typ nitid_array_get_##typ(const nitid_array arr, int64_t index) {\
  if (index < 0) {\
    index = (int64_t)arr.length + index;\
  }\
  if (index < 0 || index >= arr.length) {\
    fprintf(stderr, "Index out of bounds: %ld (length: %zu)\n", (long)index, arr.length);\
    exit(1);\
  }\
  return ((typ *)arr.data)[index];\
}

NITID_ARRAY_FROM_LIT(i8);
NITID_ARRAY_FROM_LIT(i16);
NITID_ARRAY_FROM_LIT(i32);
NITID_ARRAY_FROM_LIT(i64);
NITID_ARRAY_FROM_LIT(i128);

NITID_ARRAY_FROM_LIT(u8);
NITID_ARRAY_FROM_LIT(u16);
NITID_ARRAY_FROM_LIT(u32);
NITID_ARRAY_FROM_LIT(u64);
NITID_ARRAY_FROM_LIT(u128);

NITID_ARRAY_FROM_LIT(f32);
NITID_ARRAY_FROM_LIT(f64);
NITID_ARRAY_FROM_LIT(bool);

NITID_ARRAY_GET(i8);
NITID_ARRAY_GET(i16);
NITID_ARRAY_GET(i32);
NITID_ARRAY_GET(i64);
NITID_ARRAY_GET(i128);

NITID_ARRAY_GET(u8);
NITID_ARRAY_GET(u16);
NITID_ARRAY_GET(u32);
NITID_ARRAY_GET(u64);
NITID_ARRAY_GET(u128);

NITID_ARRAY_GET(f32);
NITID_ARRAY_GET(f64);
NITID_ARRAY_GET(bool);

size_t nitid_array_size(nitid_array arr);

/**
 * Allocate a dynamically-sized array of `count` zero-initialized
 * elements, each `elem_size` bytes wide.
 */
nitid_array nitid_array_zeros(size_t elem_size, size_t count);

/**
 * Resize a dynamic array in place to `new_len` elements.
 *
 * - Growing: the new tail elements are zero-filled.
 * - Shrinking: the array is truncated to its first `new_len` elements.
 * - Resizing to 0 frees the backing storage and resets the array.
 *
 * `arr` must have a valid `elem_size` (arrays declared without an
 * initializer get it from their declared element type).
 */
void nitid_array_resize(nitid_array *arr, size_t new_len);

#endif // NITID_ARRAY_H
