#ifndef NITID_TYPES_H
#define NITID_TYPES_H

#include <stdint.h>

typedef int8_t i8;
typedef int16_t i16;
typedef int32_t i32;
typedef int64_t i64;

typedef uint8_t u8;
typedef uint16_t u16;
typedef uint32_t u32;
typedef uint64_t u64;

typedef float f32;
typedef double f64;

// MSVC does not support __int128
#if defined(_MSC_VER) && !defined(__llvm__) && !defined(__INTEL_COMPILER)
  // No 128 bit integers math
#else
  typedef __int128 i128;
  typedef unsigned __int128 u128;
#endif

#endif //NITID_TYPES_H
