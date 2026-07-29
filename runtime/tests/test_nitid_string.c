#include "nitid_test.h"
#include "../nitid_string.h"

TEST(string_from_empty) {
    nitid_string s = nitid_string_from("");
    ASSERT_NOT_NULL(s.data);
    ASSERT_EQ(s.len, 0);
    ASSERT_EQ(s.cap, 1);
    ASSERT_STR_EQ(s.data, "");
    nitid_string_free(&s);
}

TEST(string_from_hello) {
    nitid_string s = nitid_string_from("hello");
    ASSERT_NOT_NULL(s.data);
    ASSERT_EQ(s.len, 5);
    ASSERT_EQ(s.cap, 6);
    ASSERT_STR_EQ(s.data, "hello");
    nitid_string_free(&s);
}

TEST(string_from_n_partial) {
    nitid_string s = nitid_string_from_n("hello world", 5);
    ASSERT_EQ(s.len, 5);
    ASSERT_EQ(s.cap, 6);
    ASSERT_STR_EQ(s.data, "hello");
    nitid_string_free(&s);
}

TEST(string_from_n_full) {
    nitid_string s = nitid_string_from_n("hello", 5);
    ASSERT_EQ(s.len, 5);
    ASSERT_STR_EQ(s.data, "hello");
    nitid_string_free(&s);
}

TEST(string_from_n_zero) {
    nitid_string s = nitid_string_from_n("hello", 0);
    ASSERT_EQ(s.len, 0);
    ASSERT_EQ(s.cap, 1);
    ASSERT_STR_EQ(s.data, "");
    nitid_string_free(&s);
}

TEST(string_clone) {
    nitid_string orig = nitid_string_from("clone me");
    nitid_string copy = nitid_string_clone(&orig);
    ASSERT_NOT_NULL(copy.data);
    ASSERT(copy.data != orig.data);
    ASSERT_STR_EQ(copy.data, "clone me");
    ASSERT_EQ(copy.len, orig.len);
    ASSERT_EQ(copy.cap, orig.cap);
    nitid_string_free(&orig);
    nitid_string_free(&copy);
}

TEST(string_clone_empty) {
    nitid_string orig = nitid_string_from("");
    nitid_string copy = nitid_string_clone(&orig);
    ASSERT_STR_EQ(copy.data, "");
    ASSERT_EQ(copy.len, 0);
    nitid_string_free(&orig);
    nitid_string_free(&copy);
}

TEST(string_at_valid) {
    nitid_string s = nitid_string_from("abc");
    ASSERT_EQ(nitid_string_at(&s, 0), 'a');
    ASSERT_EQ(nitid_string_at(&s, 1), 'b');
    ASSERT_EQ(nitid_string_at(&s, 2), 'c');
    nitid_string_free(&s);
}

TEST(string_at_out_of_bounds) {
    nitid_string s = nitid_string_from("abc");
    ASSERT_EQ(nitid_string_at(&s, 3), '\0');
    ASSERT_EQ(nitid_string_at(&s, 100), '\0');
    nitid_string_free(&s);
}

TEST(string_at_empty) {
    nitid_string s = nitid_string_from("");
    ASSERT_EQ(nitid_string_at(&s, 0), '\0');
    nitid_string_free(&s);
}

TEST(string_append_within_capacity) {
    nitid_string s = nitid_string_from_n("ab", 2);
    nitid_string_append(&s, "c", 1);
    ASSERT_STR_EQ(s.data, "abc");
    ASSERT_EQ(s.len, 3);
    nitid_string_free(&s);
}

TEST(string_append_multiple) {
    nitid_string s = nitid_string_from("");
    nitid_string_append(&s, "a", 1);
    ASSERT_STR_EQ(s.data, "a");
    nitid_string_append(&s, "b", 1);
    ASSERT_STR_EQ(s.data, "ab");
    nitid_string_append(&s, "c", 1);
    ASSERT_STR_EQ(s.data, "abc");
    nitid_string_free(&s);
}

TEST(string_append_beyond_capacity) {
    nitid_string s = nitid_string_from_n("abc", 3);
    ASSERT_EQ(s.cap, 4);
    nitid_string_append(&s, "defgh", 5);
    ASSERT_STR_EQ(s.data, "abcdefgh");
    ASSERT_EQ(s.len, 8);
    ASSERT(s.cap >= 9);
    nitid_string_free(&s);
}

TEST(string_append_large) {
    nitid_string s = nitid_string_from("");
    char buf[1024];
    memset(buf, 'x', 1023);
    buf[1023] = '\0';
    nitid_string_append(&s, buf, 1023);
    ASSERT_EQ(s.len, 1023);
    ASSERT_STR_EQ(s.data, buf);
    nitid_string_free(&s);
}

TEST(string_free_sets_null) {
    nitid_string s = nitid_string_from("test");
    nitid_string_free(&s);
    ASSERT_NULL(s.data);
    ASSERT_EQ(s.len, 0);
    ASSERT_EQ(s.cap, 0);
}

TEST(string_free_twice) {
    nitid_string s = nitid_string_from("test");
    nitid_string_free(&s);
    nitid_string_free(&s);
    ASSERT_NULL(s.data);
}

TEST(string_at_cp_ascii) {
    nitid_string s = nitid_string_from("abc");
    ASSERT_EQ(nitid_string_at_cp(&s, 0), 'a');
    ASSERT_EQ(nitid_string_at_cp(&s, 1), 'b');
    ASSERT_EQ(nitid_string_at_cp(&s, 2), 'c');
    ASSERT_EQ(nitid_string_at_cp(&s, 3), 0);
    nitid_string_free(&s);
}

TEST(string_at_cp_multibyte) {
    nitid_string s = nitid_string_from("\xC3\xA9\xC3\xA0\xC3\xB9"); // é à ù
    ASSERT_EQ(nitid_string_at_cp(&s, 0), 0x00E9);
    ASSERT_EQ(nitid_string_at_cp(&s, 1), 0x00E0);
    ASSERT_EQ(nitid_string_at_cp(&s, 2), 0x00F9);
    ASSERT_EQ(nitid_string_at_cp(&s, 3), 0);
    nitid_string_free(&s);
}

TEST(string_at_cp_negative) {
    nitid_string s = nitid_string_from("abc");
    ASSERT_EQ(nitid_string_at_cp(&s, -1), 'c');
    ASSERT_EQ(nitid_string_at_cp(&s, -2), 'b');
    ASSERT_EQ(nitid_string_at_cp(&s, -3), 'a');
    ASSERT_EQ(nitid_string_at_cp(&s, -4), 0);
    nitid_string_free(&s);
}

TEST(string_concat_empty) {
    nitid_string a = nitid_string_from("");
    nitid_string b = nitid_string_from("");
    nitid_string r = nitid_string_concat(&a, &b);
    ASSERT_EQ(r.len, 0);
    ASSERT_STR_EQ(r.data, "");
    nitid_string_free(&a);
    nitid_string_free(&b);
    nitid_string_free(&r);
}

TEST(string_concat_nonempty) {
    nitid_string a = nitid_string_from("Hello");
    nitid_string b = nitid_string_from(" World");
    nitid_string r = nitid_string_concat(&a, &b);
    ASSERT_EQ(r.len, 11);
    ASSERT_STR_EQ(r.data, "Hello World");
    nitid_string_free(&a);
    nitid_string_free(&b);
    nitid_string_free(&r);
}

TEST(string_eq_equal) {
    nitid_string a = nitid_string_from("hello");
    nitid_string b = nitid_string_from("hello");
    ASSERT(nitid_string_eq(&a, &b));
    ASSERT(!nitid_string_ne(&a, &b));
    nitid_string_free(&a);
    nitid_string_free(&b);
}

TEST(string_eq_different) {
    nitid_string a = nitid_string_from("hello");
    nitid_string b = nitid_string_from("world");
    ASSERT(!nitid_string_eq(&a, &b));
    ASSERT(nitid_string_ne(&a, &b));
    nitid_string_free(&a);
    nitid_string_free(&b);
}

TEST(string_lt_gt) {
    nitid_string a = nitid_string_from("alpha");
    nitid_string b = nitid_string_from("beta");
    ASSERT(nitid_string_lt(&a, &b));
    ASSERT(!nitid_string_lt(&b, &a));
    ASSERT(nitid_string_gt(&b, &a));
    ASSERT(!nitid_string_gt(&a, &b));
    ASSERT(nitid_string_le(&a, &b));
    ASSERT(nitid_string_le(&a, &a));
    ASSERT(nitid_string_ge(&b, &a));
    ASSERT(nitid_string_ge(&a, &a));
    nitid_string_free(&a);
    nitid_string_free(&b);
}

void register_string_tests(void) {
    RUN_TEST(string_from_empty);
    RUN_TEST(string_from_hello);
    RUN_TEST(string_from_n_partial);
    RUN_TEST(string_from_n_full);
    RUN_TEST(string_from_n_zero);
    RUN_TEST(string_clone);
    RUN_TEST(string_clone_empty);
    RUN_TEST(string_at_valid);
    RUN_TEST(string_at_out_of_bounds);
    RUN_TEST(string_at_empty);
    RUN_TEST(string_append_within_capacity);
    RUN_TEST(string_append_multiple);
    RUN_TEST(string_append_beyond_capacity);
    RUN_TEST(string_append_large);
    RUN_TEST(string_free_sets_null);
    RUN_TEST(string_free_twice);
    RUN_TEST(string_at_cp_ascii);
    RUN_TEST(string_at_cp_multibyte);
    RUN_TEST(string_at_cp_negative);
    RUN_TEST(string_concat_empty);
    RUN_TEST(string_concat_nonempty);
    RUN_TEST(string_eq_equal);
    RUN_TEST(string_eq_different);
    RUN_TEST(string_lt_gt);
}
