#include "nitid_test.h"
#include "../nitid_array.h"

TEST(array_from_lit_empty) {
    int32_t vals[] = {0};
    nitid_array a = nitid_array_from_lit_i32(0, vals);
    ASSERT_EQ(a.length, 0);
    ASSERT_EQ(a.capacity, 0);
    ASSERT_EQ(a.elem_size, sizeof(int32_t));
    ASSERT_NOT_NULL(a.data);
    ASSERT_EQ(nitid_array_size(a), 0);
    free(a.data);
}

TEST(array_from_lit_single) {
    int32_t vals[] = {42};
    nitid_array a = nitid_array_from_lit_i32(1, vals);
    ASSERT_EQ(a.length, 1);
    ASSERT_EQ(a.capacity, 1);
    ASSERT_EQ(a.elem_size, sizeof(int32_t));
    ASSERT_EQ(nitid_array_size(a), 1);
    ASSERT_EQ(nitid_array_get_i32(a, 0), 42);
    free(a.data);
}

TEST(array_from_lit_multiple) {
    int32_t vals[] = {10, 20, 30, 40, 50};
    nitid_array a = nitid_array_from_lit_i32(5, vals);
    ASSERT_EQ(a.length, 5);
    ASSERT_EQ(nitid_array_size(a), 5);
    ASSERT_EQ(nitid_array_get_i32(a, 0), 10);
    ASSERT_EQ(nitid_array_get_i32(a, 2), 30);
    ASSERT_EQ(nitid_array_get_i32(a, 4), 50);
    free(a.data);
}

TEST(array_get_negative_index) {
    int32_t vals[] = {10, 20, 30};
    nitid_array a = nitid_array_from_lit_i32(3, vals);
    ASSERT_EQ(nitid_array_get_i32(a, -1), 30);
    ASSERT_EQ(nitid_array_get_i32(a, -2), 20);
    ASSERT_EQ(nitid_array_get_i32(a, -3), 10);
    free(a.data);
}

TEST(array_i64) {
    int64_t vals[] = {1000000000000LL, 2000000000000LL};
    nitid_array a = nitid_array_from_lit_i64(2, vals);
    ASSERT_EQ(a.elem_size, sizeof(int64_t));
    ASSERT_EQ(nitid_array_size(a), 2);
    ASSERT_EQ(nitid_array_get_i64(a, 0), 1000000000000LL);
    ASSERT_EQ(nitid_array_get_i64(a, 1), 2000000000000LL);
    free(a.data);
}

TEST(array_u8) {
    uint8_t vals[] = {10, 20, 30};
    nitid_array a = nitid_array_from_lit_u8(3, vals);
    ASSERT_EQ(a.elem_size, sizeof(uint8_t));
    ASSERT_EQ(nitid_array_get_u8(a, 0), 10);
    ASSERT_EQ(nitid_array_get_u8(a, 2), 30);
    free(a.data);
}

TEST(array_f64) {
    double vals[] = {1.5, 2.5, 3.5};
    nitid_array a = nitid_array_from_lit_f64(3, vals);
    ASSERT_EQ(a.elem_size, sizeof(double));
    ASSERT_EQ(nitid_array_get_f64(a, 0), 1.5);
    ASSERT_EQ(nitid_array_get_f64(a, 1), 2.5);
    ASSERT_EQ(nitid_array_get_f64(a, 2), 3.5);
    free(a.data);
}

TEST(array_bool) {
    bool vals[] = {true, false, true};
    nitid_array a = nitid_array_from_lit_bool(3, vals);
    ASSERT_EQ(a.elem_size, sizeof(bool));
    ASSERT_EQ(nitid_array_get_bool(a, 0), true);
    ASSERT_EQ(nitid_array_get_bool(a, 1), false);
    ASSERT_EQ(nitid_array_get_bool(a, 2), true);
    free(a.data);
}

TEST(array_data_independent) {
    int32_t vals[] = {1, 2, 3};
    nitid_array a = nitid_array_from_lit_i32(3, vals);
    vals[0] = 999;
    ASSERT_EQ(nitid_array_get_i32(a, 0), 1);
    free(a.data);
}

TEST(array_resize_grow_zero_fills) {
    int32_t vals[] = {1, 2, 3};
    nitid_array a = nitid_array_from_lit_i32(3, vals);
    nitid_array_resize(&a, 6);
    ASSERT_EQ(a.length, 6);
    ASSERT_EQ(a.capacity, 6);
    ASSERT_EQ(nitid_array_get_i32(a, 0), 1);
    ASSERT_EQ(nitid_array_get_i32(a, 2), 3);
    ASSERT_EQ(nitid_array_get_i32(a, 3), 0);
    ASSERT_EQ(nitid_array_get_i32(a, 5), 0);
    free(a.data);
}

TEST(array_resize_shrunk_keeps_prefix) {
    int32_t vals[] = {1, 2, 3, 4, 5};
    nitid_array a = nitid_array_from_lit_i32(5, vals);
    nitid_array_resize(&a, 2);
    ASSERT_EQ(nitid_array_size(a), 2);
    ASSERT_EQ(nitid_array_get_i32(a, 0), 1);
    ASSERT_EQ(nitid_array_get_i32(a, 1), 2);
    nitid_array_resize(&a, 4);
    ASSERT_EQ(nitid_array_size(a), 4);
    ASSERT_EQ(nitid_array_get_i32(a, 0), 1);
    ASSERT_EQ(nitid_array_get_i32(a, 2), 0); // regrown tail is zeroed
    free(a.data);
}

TEST(array_resize_to_zero_then_regrow) {
    int32_t vals[] = {7, 8};
    nitid_array a = nitid_array_from_lit_i32(2, vals);
    nitid_array_resize(&a, 0);
    ASSERT_EQ(nitid_array_size(a), 0);
    ASSERT_NULL(a.data);
    nitid_array_resize(&a, 3);
    ASSERT_EQ(nitid_array_size(a), 3);
    ASSERT_EQ(nitid_array_get_i32(a, 0), 0);
    free(a.data);
}

TEST(array_resize_empty_decl) {
    // Mirrors `a := int[10];` — declared without initializer.
    nitid_array a = {.elem_size = sizeof(int64_t)};
    nitid_array_resize(&a, 10);
    ASSERT_EQ(nitid_array_size(a), 10);
    ASSERT_EQ(a.elem_size, sizeof(int64_t));
    ASSERT_EQ(nitid_array_get_i64(a, 9), 0);
    free(a.data);
}

TEST(array_zeros) {
    nitid_array a = nitid_array_zeros(sizeof(int32_t), 4);
    ASSERT_EQ(a.length, 4);
    ASSERT_EQ(a.capacity, 4);
    ASSERT_EQ(a.elem_size, sizeof(int32_t));
    ASSERT_NOT_NULL(a.data);
    for (size_t i = 0; i < 4; i++) {
        ASSERT_EQ(nitid_array_get_i32(a, (int64_t)i), 0);
    }
    free(a.data);
}

TEST(array_resize_same_size_noop) {
    int32_t vals[] = {5};
    nitid_array a = nitid_array_from_lit_i32(1, vals);
    nitid_array_resize(&a, 1);
    ASSERT_EQ(nitid_array_get_i32(a, 0), 5);
    ASSERT_EQ(nitid_array_size(a), 1);
    free(a.data);
}

void register_array_tests(void) {
    RUN_TEST(array_from_lit_empty);
    RUN_TEST(array_from_lit_single);
    RUN_TEST(array_from_lit_multiple);
    RUN_TEST(array_get_negative_index);
    RUN_TEST(array_i64);
    RUN_TEST(array_u8);
    RUN_TEST(array_f64);
    RUN_TEST(array_bool);
    RUN_TEST(array_data_independent);
    RUN_TEST(array_resize_grow_zero_fills);
    RUN_TEST(array_resize_shrunk_keeps_prefix);
    RUN_TEST(array_resize_to_zero_then_regrow);
    RUN_TEST(array_resize_empty_decl);
    RUN_TEST(array_resize_same_size_noop);
    RUN_TEST(array_zeros);
}
