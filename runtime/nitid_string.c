#include "nitid_string.h"
#include <stdlib.h>
#include <string.h>
#include <stdint.h>

nitid_string nitid_string_from(const char *s) {
    const size_t len = strlen(s);
    return nitid_string_from_n(s, len);
}

nitid_string nitid_string_from_n(const char *s, const size_t n) {
    nitid_string result;
    result.data = (char *)malloc(n + 1);
    if (result.data) {
        memcpy(result.data, s, n);
        result.data[n] = '\0';
        result.len = n;
        result.cap = n + 1;
    } else {
        result.data = NULL;
        result.len = 0;
        result.cap = 0;
    }
    return result;
}

nitid_string nitid_string_clone(const nitid_string *s) {
    return nitid_string_from_n(s->data, s->len);
}

void nitid_string_free(nitid_string *s) {
    if (s->data) {
        free(s->data);
        s->data = NULL;
    }
    s->len = 0;
    s->cap = 0;
}

char nitid_string_at(const nitid_string *s, const size_t index) {
    if (index < s->len) {
        return s->data[index];
    }
    return '\0';
}

void nitid_string_append(nitid_string *s, const char *data, const size_t n) {
    size_t new_len = s->len + n;
    if (new_len + 1 > s->cap) {
        size_t new_cap = s->cap * 2;
        if (new_cap < new_len + 1) {
            new_cap = new_len + 1;
        }
        char *new_data = realloc(s->data, new_cap);
        if (new_data) {
            s->data = new_data;
            s->cap = new_cap;
        } else {
            return;
        }
    }
    memcpy(s->data + s->len, data, n);
    s->data[new_len] = '\0';
    s->len = new_len;
}

static uint32_t utf8_decode_at(const char *data, size_t len, size_t *pos) {
    if (*pos >= len) return 0;
    uint8_t b = (uint8_t)data[*pos];
    uint32_t cp;
    size_t extra;
    if (b < 0x80) { cp = b; extra = 0; }
    else if ((b & 0xE0) == 0xC0) { cp = b & 0x1F; extra = 1; }
    else if ((b & 0xF0) == 0xE0) { cp = b & 0x0F; extra = 2; }
    else if ((b & 0xF8) == 0xF0) { cp = b & 0x07; extra = 3; }
    else { *pos += 1; return 0xFFFD; }
    if (*pos + extra >= len) return 0xFFFD;
    for (size_t i = 0; i < extra; i++) {
        (*pos)++;
        cp = (cp << 6) | ((uint8_t)data[*pos] & 0x3F);
    }
    *pos += 1;
    return cp;
}

uint32_t nitid_string_at_cp(const nitid_string *s, int64_t index) {
    if (index < 0) {
        size_t cp_count = 0;
        size_t pos = 0;
        while (pos < s->len) { utf8_decode_at(s->data, s->len, &pos); cp_count++; }
        index = (int64_t)cp_count + index;
    }
    if (index < 0) return 0;
    size_t pos = 0;
    for (int64_t i = 0; i < index; i++) {
        if (pos >= s->len) return 0;
        utf8_decode_at(s->data, s->len, &pos);
    }
    if (pos >= s->len) return 0;
    return utf8_decode_at(s->data, s->len, &pos);
}

nitid_string nitid_string_concat(const nitid_string *a, const nitid_string *b) {
    nitid_string result;
    result.len = a->len + b->len;
    result.cap = result.len + 1;
    result.data = (char*)malloc(result.cap);
    if (result.data) {
        memcpy(result.data, a->data, a->len);
        memcpy(result.data + a->len, b->data, b->len);
        result.data[result.len] = '\0';
    } else {
        result.len = 0;
        result.cap = 0;
    }
    return result;
}

bool nitid_string_eq(const nitid_string *a, const nitid_string *b) {
    if (a->len != b->len) return false;
    return memcmp(a->data, b->data, a->len) == 0;
}

bool nitid_string_ne(const nitid_string *a, const nitid_string *b) {
    return !nitid_string_eq(a, b);
}

static int nitid_string_cmp(const nitid_string *a, const nitid_string *b) {
    size_t i = 0, j = 0;
    while (i < a->len && j < b->len) {
        uint32_t ca = utf8_decode_at(a->data, a->len, &i);
        uint32_t cb = utf8_decode_at(b->data, b->len, &j);
        if (ca != cb) return ca < cb ? -1 : 1;
    }
    if (i < a->len) return 1;
    if (j < b->len) return -1;
    return 0;
}

bool nitid_string_lt(const nitid_string *a, const nitid_string *b) {
    return nitid_string_cmp(a, b) < 0;
}

bool nitid_string_le(const nitid_string *a, const nitid_string *b) {
    return nitid_string_cmp(a, b) <= 0;
}

bool nitid_string_gt(const nitid_string *a, const nitid_string *b) {
    return nitid_string_cmp(a, b) > 0;
}

bool nitid_string_ge(const nitid_string *a, const nitid_string *b) {
    return nitid_string_cmp(a, b) >= 0;
}
