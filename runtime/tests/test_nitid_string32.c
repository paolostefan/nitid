#include "nitid_test.h"
#include "../nitid_string32.h"

TEST(string32_from_utf8_ascii) {
    nitid_string32 s = nitid_string32_from_utf8("hello");
    ASSERT_NOT_NULL(s.data);
    ASSERT_EQ(s.len, 5);
    ASSERT_EQ(s.data[0], 'h');
    ASSERT_EQ(s.data[4], 'o');
    nitid_string32_free(&s);
}

TEST(string32_from_utf8_empty) {
    nitid_string32 s = nitid_string32_from_utf8("");
    ASSERT_NOT_NULL(s.data);
    ASSERT_EQ(s.len, 0);
    nitid_string32_free(&s);
}

TEST(string32_from_utf8_multibyte) {
    nitid_string32 s = nitid_string32_from_utf8("\xC3\xA9");
    ASSERT_EQ(s.len, 1);
    ASSERT_EQ(s.data[0], 0x00E9);
    nitid_string32_free(&s);
}

TEST(string32_from_utf8_surrogate) {
    nitid_string32 s = nitid_string32_from_utf8("\xF0\x9F\x98\x80");
    ASSERT_EQ(s.len, 1);
    ASSERT_EQ(s.data[0], 0x1F600);
    nitid_string32_free(&s);
}

TEST(string32_from_n) {
    uint32_t buf[] = { 'a', 'b', 'c' };
    nitid_string32 s = nitid_string32_from_n(buf, 3);
    ASSERT_EQ(s.len, 3);
    ASSERT_EQ(s.data[0], 'a');
    ASSERT_EQ(s.data[2], 'c');
    nitid_string32_free(&s);
}

TEST(string32_clone) {
    nitid_string32 orig = nitid_string32_from_utf8("hello");
    nitid_string32 copy = nitid_string32_clone(&orig);
    ASSERT_NOT_NULL(copy.data);
    ASSERT(copy.data != orig.data);
    ASSERT_EQ(copy.len, orig.len);
    ASSERT_EQ(copy.data[0], 'h');
    nitid_string32_free(&orig);
    nitid_string32_free(&copy);
}

TEST(string32_free_sets_null) {
    nitid_string32 s = nitid_string32_from_utf8("test");
    nitid_string32_free(&s);
    ASSERT_NULL(s.data);
    ASSERT_EQ(s.len, 0);
    ASSERT_EQ(s.cap, 0);
}

TEST(string32_at) {
    nitid_string32 s = nitid_string32_from_utf8("abc");
    ASSERT_EQ(nitid_string32_at(&s, 0), 'a');
    ASSERT_EQ(nitid_string32_at(&s, 1), 'b');
    ASSERT_EQ(nitid_string32_at(&s, 2), 'c');
    ASSERT_EQ(nitid_string32_at(&s, 3), 0);
    nitid_string32_free(&s);
}

TEST(string32_at_surrogate) {
    nitid_string32 s = nitid_string32_from_utf8("\xF0\x9F\x98\x80");
    ASSERT_EQ(nitid_string32_at(&s, 0), 0x1F600);
    nitid_string32_free(&s);
}

TEST(string32_len) {
    nitid_string32 s = nitid_string32_from_utf8("abcdef");
    ASSERT_EQ(nitid_string32_len(&s), 6);
    nitid_string32_free(&s);
}

TEST(string32_len_surrogate) {
    nitid_string32 s = nitid_string32_from_utf8("\xF0\x9F\x98\x80");
    ASSERT_EQ(nitid_string32_len(&s), 1);
    nitid_string32_free(&s);
}

TEST(string32_len_empty) {
    nitid_string32 s = nitid_string32_from_utf8("");
    ASSERT_EQ(nitid_string32_len(&s), 0);
    nitid_string32_free(&s);
}

TEST(string32_to_utf8_ascii) {
    nitid_string32 s = nitid_string32_from_utf8("hello");
    nitid_string utf8 = nitid_string32_to_utf8(&s);
    ASSERT_STR_EQ(utf8.data, "hello");
    nitid_string_free(&utf8);
    nitid_string32_free(&s);
}

TEST(string32_to_utf8_surrogate) {
    nitid_string32 s = nitid_string32_from_utf8("\xF0\x9F\x98\x80");
    nitid_string utf8 = nitid_string32_to_utf8(&s);
    ASSERT_STR_EQ(utf8.data, "\xF0\x9F\x98\x80");
    nitid_string_free(&utf8);
    nitid_string32_free(&s);
}

TEST(string32_from_utf16_ascii) {
    uint16_t buf[] = { 'a', 'b', 'c' };
    nitid_string32 s = nitid_string32_from_utf16(buf, 3);
    ASSERT_EQ(s.len, 3);
    ASSERT_EQ(s.data[0], 'a');
    ASSERT_EQ(s.data[2], 'c');
    nitid_string32_free(&s);
}

TEST(string32_from_utf16_surrogate) {
    uint16_t buf[] = { 0xD83D, 0xDE00 };
    nitid_string32 s = nitid_string32_from_utf16(buf, 2);
    ASSERT_EQ(s.len, 1);
    ASSERT_EQ(s.data[0], 0x1F600);
    nitid_string32_free(&s);
}

TEST(string32_to_utf16_ascii) {
    nitid_string32 s = nitid_string32_from_utf8("abc");
    size_t out_len;
    uint16_t *utf16 = nitid_string32_to_utf16(&s, &out_len);
    ASSERT_EQ(out_len, 3);
    ASSERT_EQ(utf16[0], 'a');
    ASSERT_EQ(utf16[2], 'c');
    free(utf16);
    nitid_string32_free(&s);
}

TEST(string32_to_utf16_surrogate) {
    nitid_string32 s = nitid_string32_from_utf8("\xF0\x9F\x98\x80");
    size_t out_len;
    uint16_t *utf16 = nitid_string32_to_utf16(&s, &out_len);
    ASSERT_EQ(out_len, 2);
    ASSERT_EQ(utf16[0], 0xD83D);
    ASSERT_EQ(utf16[1], 0xDE00);
    free(utf16);
    nitid_string32_free(&s);
}

TEST(string32_concat) {
    nitid_string32 a = nitid_string32_from_utf8("Hello");
    nitid_string32 b = nitid_string32_from_utf8(" World");
    nitid_string32 r = nitid_string32_concat(&a, &b);
    ASSERT_EQ(r.len, 11);
    ASSERT_EQ(r.data[0], 'H');
    ASSERT_EQ(r.data[10], 'd');
    nitid_string32_free(&a);
    nitid_string32_free(&b);
    nitid_string32_free(&r);
}

TEST(string32_at_cp_ascii) {
    nitid_string32 s = nitid_string32_from_utf8("abc");
    ASSERT_EQ(nitid_string32_at_cp(&s, 0), 'a');
    ASSERT_EQ(nitid_string32_at_cp(&s, 1), 'b');
    ASSERT_EQ(nitid_string32_at_cp(&s, 2), 'c');
    ASSERT_EQ(nitid_string32_at_cp(&s, 3), 0);
    nitid_string32_free(&s);
}

TEST(string32_at_cp_negative) {
    nitid_string32 s = nitid_string32_from_utf8("abc");
    ASSERT_EQ(nitid_string32_at_cp(&s, -1), 'c');
    ASSERT_EQ(nitid_string32_at_cp(&s, -2), 'b');
    ASSERT_EQ(nitid_string32_at_cp(&s, -3), 'a');
    ASSERT_EQ(nitid_string32_at_cp(&s, -4), 0);
    nitid_string32_free(&s);
}

TEST(string32_eq) {
    nitid_string32 a = nitid_string32_from_utf8("hello");
    nitid_string32 b = nitid_string32_from_utf8("hello");
    nitid_string32 c = nitid_string32_from_utf8("world");
    ASSERT(nitid_string32_eq(&a, &b));
    ASSERT(!nitid_string32_ne(&a, &b));
    ASSERT(!nitid_string32_eq(&a, &c));
    ASSERT(nitid_string32_ne(&a, &c));
    nitid_string32_free(&a);
    nitid_string32_free(&b);
    nitid_string32_free(&c);
}

TEST(string32_lt_gt) {
    nitid_string32 a = nitid_string32_from_utf8("alpha");
    nitid_string32 b = nitid_string32_from_utf8("beta");
    ASSERT(nitid_string32_lt(&a, &b));
    ASSERT(nitid_string32_gt(&b, &a));
    ASSERT(nitid_string32_le(&a, &b));
    ASSERT(nitid_string32_ge(&b, &a));
    nitid_string32_free(&a);
    nitid_string32_free(&b);
}

void register_string32_tests(void) {
    RUN_TEST(string32_from_utf8_ascii);
    RUN_TEST(string32_from_utf8_empty);
    RUN_TEST(string32_from_utf8_multibyte);
    RUN_TEST(string32_from_utf8_surrogate);
    RUN_TEST(string32_from_n);
    RUN_TEST(string32_clone);
    RUN_TEST(string32_free_sets_null);
    RUN_TEST(string32_at);
    RUN_TEST(string32_at_surrogate);
    RUN_TEST(string32_len);
    RUN_TEST(string32_len_surrogate);
    RUN_TEST(string32_len_empty);
    RUN_TEST(string32_to_utf8_ascii);
    RUN_TEST(string32_to_utf8_surrogate);
    RUN_TEST(string32_from_utf16_ascii);
    RUN_TEST(string32_from_utf16_surrogate);
    RUN_TEST(string32_to_utf16_ascii);
    RUN_TEST(string32_to_utf16_surrogate);
    RUN_TEST(string32_concat);
    RUN_TEST(string32_at_cp_ascii);
    RUN_TEST(string32_at_cp_negative);
    RUN_TEST(string32_eq);
    RUN_TEST(string32_lt_gt);
}
