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
