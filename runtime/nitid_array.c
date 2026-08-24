#include "nitid_array.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

NITID_ARRAY_FROM_LIT_IMPL(i8, int8_t)
NITID_ARRAY_FROM_LIT_IMPL(i16, int16_t)
NITID_ARRAY_FROM_LIT_IMPL(i32, int32_t)
NITID_ARRAY_FROM_LIT_IMPL(i64, int64_t)
NITID_ARRAY_FROM_LIT_IMPL(i128, __int128)

NITID_ARRAY_FROM_LIT_IMPL(u8, uint8_t)
NITID_ARRAY_FROM_LIT_IMPL(u16, uint16_t)
NITID_ARRAY_FROM_LIT_IMPL(u32, uint32_t)
NITID_ARRAY_FROM_LIT_IMPL(u64, uint64_t)
NITID_ARRAY_FROM_LIT_IMPL(u128, unsigned __int128)

NITID_ARRAY_GET_IMPL(i8, int8_t);
NITID_ARRAY_GET_IMPL(i16, int16_t);
NITID_ARRAY_GET_IMPL(i32, int32_t);
NITID_ARRAY_GET_IMPL(i64, int64_t);
NITID_ARRAY_GET_IMPL(i128, __int128);

NITID_ARRAY_GET_IMPL(u8, uint8_t);
NITID_ARRAY_GET_IMPL(u16, uint16_t);
NITID_ARRAY_GET_IMPL(u32, uint32_t);
NITID_ARRAY_GET_IMPL(u64, uint64_t);
NITID_ARRAY_GET_IMPL(u128, unsigned __int128);

NITID_ARRAY_FROM_LIT_IMPL(f32, float)
NITID_ARRAY_FROM_LIT_IMPL(f64, double)
NITID_ARRAY_FROM_LIT_IMPL(bool, bool)
NITID_ARRAY_GET_IMPL(f32, float);
NITID_ARRAY_GET_IMPL(f64, double);
NITID_ARRAY_GET_IMPL(bool, bool);

size_t nitid_array_size(const nitid_array arr) {
    return arr.length;
}

nitid_array nitid_array_zeros(const size_t elem_size, const size_t count) {
    nitid_array arr;
    if (elem_size == 0 || count > SIZE_MAX / elem_size) {
        fprintf(stderr, "Invalid array allocation: %zu elements of size %zu\n",
                count, elem_size);
        exit(1);
    }
    arr.data = calloc(count, elem_size);
    if (arr.data == NULL && count > 0) {
        fprintf(stderr, "Out of memory allocating array of %zu elements\n", count);
        exit(1);
    }
    arr.length = count;
    arr.capacity = count;
    arr.elem_size = elem_size;
    return arr;
}

void nitid_array_resize(nitid_array *arr, const size_t new_len) {
    if (arr->elem_size == 0) {
        fprintf(stderr, "Cannot resize array with unknown element size\n");
        exit(1);
    }
    if (new_len == arr->length) {
        return;
    }
    if (new_len == 0) {
        free(arr->data);
        arr->data = NULL;
        arr->length = 0;
        arr->capacity = 0;
        return;
    }
    if (new_len > SIZE_MAX / arr->elem_size) {
        fprintf(stderr, "Array resize overflow: %zu elements of size %zu\n",
                new_len, arr->elem_size);
        exit(1);
    }
    void *new_data = realloc(arr->data, new_len * arr->elem_size);
    if (new_data == NULL) {
        fprintf(stderr, "Out of memory resizing array to %zu elements\n", new_len);
        exit(1);
    }
    arr->data = new_data;
    if (new_len > arr->length) {
        memset((char *)arr->data + arr->length * arr->elem_size, 0,
               (new_len - arr->length) * arr->elem_size);
    }
    arr->length = new_len;
    arr->capacity = new_len;
}
