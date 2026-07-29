#include "nitid_test.h"
#include "../nitid_string16.h"

TEST(string16_from_utf8_ascii) {
    nitid_string16 s = nitid_string16_from_utf8("hello");
    ASSERT_NOT_NULL(s.data);
    ASSERT_EQ(s.len, 5);
    ASSERT_EQ(s.data[0], 'h');
    ASSERT_EQ(s.data[4], 'o');
    nitid_string16_free(&s);
}

TEST(string16_from_utf8_empty) {
    nitid_string16 s = nitid_string16_from_utf8("");
    ASSERT_NOT_NULL(s.data);
    ASSERT_EQ(s.len, 0);
    nitid_string16_free(&s);
}

TEST(string16_from_utf8_multibyte) {
    nitid_string16 s = nitid_string16_from_utf8("\xC3\xA9"); // U+00E9 é
    ASSERT_NOT_NULL(s.data);
    ASSERT_EQ(s.len, 1);
    ASSERT_EQ(s.data[0], 0x00E9);
    nitid_string16_free(&s);
}

TEST(string16_from_utf8_surrogate) {
    nitid_string16 s = nitid_string16_from_utf8("\xF0\x9F\x98\x80"); // U+1F600 😀
    ASSERT_NOT_NULL(s.data);
    ASSERT_EQ(s.len, 2);
    ASSERT_EQ((s.data[0] & 0xFC00), 0xD800);
    ASSERT_EQ((s.data[1] & 0xFC00), 0xDC00);
    nitid_string16_free(&s);
}

TEST(string16_from_n) {
    uint16_t buf[] = { 'a', 'b', 'c' };
    nitid_string16 s = nitid_string16_from_n(buf, 3);
    ASSERT_NOT_NULL(s.data);
    ASSERT_EQ(s.len, 3);
    ASSERT_EQ(s.data[0], 'a');
    ASSERT_EQ(s.data[2], 'c');
    nitid_string16_free(&s);
}

TEST(string16_clone) {
    nitid_string16 orig = nitid_string16_from_utf8("hello");
    nitid_string16 copy = nitid_string16_clone(&orig);
    ASSERT_NOT_NULL(copy.data);
    ASSERT(copy.data != orig.data);
    ASSERT_EQ(copy.len, orig.len);
    ASSERT_EQ(copy.data[0], 'h');
    nitid_string16_free(&orig);
    nitid_string16_free(&copy);
}

TEST(string16_free_sets_null) {
    nitid_string16 s = nitid_string16_from_utf8("test");
    nitid_string16_free(&s);
    ASSERT_NULL(s.data);
    ASSERT_EQ(s.len, 0);
    ASSERT_EQ(s.cap, 0);
}

TEST(string16_len_ascii) {
    nitid_string16 s = nitid_string16_from_utf8("abc");
    ASSERT_EQ(nitid_string16_len(&s), 3);
    nitid_string16_free(&s);
}

TEST(string16_len_surrogate) {
    nitid_string16 s = nitid_string16_from_utf8("\xF0\x9F\x98\x80"); // U+1F600
    ASSERT_EQ(nitid_string16_len(&s), 1);
    nitid_string16_free(&s);
}

TEST(string16_len_mixed) {
    nitid_string16 s = nitid_string16_from_utf8("a\xF0\x9F\x98\x80\xC3\xA9"); // a, U+1F600, U+00E9
    ASSERT_EQ(nitid_string16_len(&s), 3);
    ASSERT_EQ(s.len, 4);
    nitid_string16_free(&s);
}

TEST(string16_at_ascii) {
    nitid_string16 s = nitid_string16_from_utf8("abc");
    ASSERT_EQ(nitid_string16_at(&s, 0), 'a');
    ASSERT_EQ(nitid_string16_at(&s, 1), 'b');
    ASSERT_EQ(nitid_string16_at(&s, 2), 'c');
    ASSERT_EQ(nitid_string16_at(&s, 3), 0);
    nitid_string16_free(&s);
}

TEST(string16_at_surrogate) {
    nitid_string16 s = nitid_string16_from_utf8("\xF0\x9F\x98\x80"); // U+1F600
    ASSERT_EQ(nitid_string16_at(&s, 0), 0x1F600);
    ASSERT_EQ(nitid_string16_at(&s, 1), 0);
    nitid_string16_free(&s);
}

TEST(string16_to_utf8_ascii) {
    nitid_string16 s = nitid_string16_from_utf8("hello");
    nitid_string utf8 = nitid_string16_to_utf8(&s);
    ASSERT_STR_EQ(utf8.data, "hello");
    ASSERT_EQ(utf8.len, 5);
    nitid_string_free(&utf8);
    nitid_string16_free(&s);
}

TEST(string16_to_utf8_surrogate) {
    nitid_string16 s = nitid_string16_from_utf8("\xF0\x9F\x98\x80");
    nitid_string utf8 = nitid_string16_to_utf8(&s);
    ASSERT_STR_EQ(utf8.data, "\xF0\x9F\x98\x80");
    nitid_string_free(&utf8);
    nitid_string16_free(&s);
}

TEST(string16_from_utf32) {
    uint32_t cp[] = { 0x1F600, 0x00E9, 'A' };
    nitid_string16 s = nitid_string16_from_utf32(cp, 3);
    ASSERT_EQ(s.len, 4);
    ASSERT_EQ(nitid_string16_len(&s), 3);
    ASSERT_EQ(nitid_string16_at(&s, 0), 0x1F600);
    ASSERT_EQ(nitid_string16_at(&s, 1), 0x00E9);
    ASSERT_EQ(nitid_string16_at(&s, 2), 'A');
    nitid_string16_free(&s);
}

TEST(string16_to_utf32) {
    nitid_string16 s = nitid_string16_from_utf8("a\xF0\x9F\x98\x80\xC3\xA9");
    size_t out_len;
    uint32_t *utf32 = nitid_string16_to_utf32(&s, &out_len);
    ASSERT_EQ(out_len, 3);
    ASSERT_EQ(utf32[0], 'a');
    ASSERT_EQ(utf32[1], 0x1F600);
    ASSERT_EQ(utf32[2], 0x00E9);
    free(utf32);
    nitid_string16_free(&s);
}

TEST(string16_concat) {
    nitid_string16 a = nitid_string16_from_utf8("Hello");
    nitid_string16 b = nitid_string16_from_utf8(" World");
    nitid_string16 r = nitid_string16_concat(&a, &b);
    ASSERT_EQ(r.len, 11);
    ASSERT_EQ(r.data[0], 'H');
    ASSERT_EQ(r.data[10], 'd');
    nitid_string16_free(&a);
    nitid_string16_free(&b);
    nitid_string16_free(&r);
}

TEST(string16_at_cp_ascii) {
    nitid_string16 s = nitid_string16_from_utf8("abc");
    ASSERT_EQ(nitid_string16_at_cp(&s, 0), 'a');
    ASSERT_EQ(nitid_string16_at_cp(&s, 1), 'b');
    ASSERT_EQ(nitid_string16_at_cp(&s, 2), 'c');
    ASSERT_EQ(nitid_string16_at_cp(&s, 3), 0);
    nitid_string16_free(&s);
}

TEST(string16_at_cp_negative) {
    nitid_string16 s = nitid_string16_from_utf8("abc");
    ASSERT_EQ(nitid_string16_at_cp(&s, -1), 'c');
    ASSERT_EQ(nitid_string16_at_cp(&s, -2), 'b');
    ASSERT_EQ(nitid_string16_at_cp(&s, -3), 'a');
    ASSERT_EQ(nitid_string16_at_cp(&s, -4), 0);
    nitid_string16_free(&s);
}

TEST(string16_eq) {
    nitid_string16 a = nitid_string16_from_utf8("hello");
    nitid_string16 b = nitid_string16_from_utf8("hello");
    nitid_string16 c = nitid_string16_from_utf8("world");
    ASSERT(nitid_string16_eq(&a, &b));
    ASSERT(!nitid_string16_ne(&a, &b));
    ASSERT(!nitid_string16_eq(&a, &c));
    ASSERT(nitid_string16_ne(&a, &c));
    nitid_string16_free(&a);
    nitid_string16_free(&b);
    nitid_string16_free(&c);
}

TEST(string16_lt_gt) {
    nitid_string16 a = nitid_string16_from_utf8("alpha");
    nitid_string16 b = nitid_string16_from_utf8("beta");
    ASSERT(nitid_string16_lt(&a, &b));
    ASSERT(nitid_string16_gt(&b, &a));
    ASSERT(nitid_string16_le(&a, &b));
    ASSERT(nitid_string16_ge(&b, &a));
    nitid_string16_free(&a);
    nitid_string16_free(&b);
}

void register_string16_tests(void) {
    RUN_TEST(string16_from_utf8_ascii);
    RUN_TEST(string16_from_utf8_empty);
    RUN_TEST(string16_from_utf8_multibyte);
    RUN_TEST(string16_from_utf8_surrogate);
    RUN_TEST(string16_from_n);
    RUN_TEST(string16_clone);
    RUN_TEST(string16_free_sets_null);
    RUN_TEST(string16_len_ascii);
    RUN_TEST(string16_len_surrogate);
    RUN_TEST(string16_len_mixed);
    RUN_TEST(string16_at_ascii);
    RUN_TEST(string16_at_surrogate);
    RUN_TEST(string16_to_utf8_ascii);
    RUN_TEST(string16_to_utf8_surrogate);
    RUN_TEST(string16_from_utf32);
    RUN_TEST(string16_to_utf32);
    RUN_TEST(string16_concat);
    RUN_TEST(string16_at_cp_ascii);
    RUN_TEST(string16_at_cp_negative);
    RUN_TEST(string16_eq);
    RUN_TEST(string16_lt_gt);
}
