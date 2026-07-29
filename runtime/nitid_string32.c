#include "nitid_string32.h"

static uint32_t utf8_decode_cp(const char **s) {
    uint8_t b = (uint8_t)**s;
    uint32_t cp;
    size_t extra;
    if (b < 0x80) { cp = b; extra = 0; }
    else if ((b & 0xE0) == 0xC0) { cp = b & 0x1F; extra = 1; }
    else if ((b & 0xF0) == 0xE0) { cp = b & 0x0F; extra = 2; }
    else if ((b & 0xF8) == 0xF0) { cp = b & 0x07; extra = 3; }
    else { *s += 1; return 0xFFFD; }
    for (size_t i = 0; i < extra; i++) {
        (*s)++;
        if ((uint8_t)**s == 0) { return 0xFFFD; }
        cp = (cp << 6) | ((uint8_t)**s & 0x3F);
    }
    *s += 1;
    if (cp > 0x10FFFF) return 0xFFFD;
    if (cp >= 0xD800 && cp <= 0xDFFF) return 0xFFFD;
    return cp;
}

static void utf8_encode_cp(uint32_t cp, char **out) {
    if (cp < 0x80) {
        *(*out)++ = (char)cp;
    } else if (cp < 0x800) {
        *(*out)++ = (char)(0xC0 | (cp >> 6));
        *(*out)++ = (char)(0x80 | (cp & 0x3F));
    } else if (cp < 0x10000) {
        *(*out)++ = (char)(0xE0 | (cp >> 12));
        *(*out)++ = (char)(0x80 | ((cp >> 6) & 0x3F));
        *(*out)++ = (char)(0x80 | (cp & 0x3F));
    } else {
        *(*out)++ = (char)(0xF0 | (cp >> 18));
        *(*out)++ = (char)(0x80 | ((cp >> 12) & 0x3F));
        *(*out)++ = (char)(0x80 | ((cp >> 6) & 0x3F));
        *(*out)++ = (char)(0x80 | (cp & 0x3F));
    }
}

static size_t utf8_cp_count(const char *s) {
    size_t n = 0;
    while (*s) { utf8_decode_cp(&s); n++; }
    return n;
}

static size_t utf16_cp_count(const uint16_t *s, size_t n) {
    size_t count = 0;
    for (size_t i = 0; i < n; i++) {
        if (s[i] >= 0xD800 && s[i] <= 0xDBFF && i + 1 < n &&
            s[i + 1] >= 0xDC00 && s[i + 1] <= 0xDFFF) {
            i++;
        }
        count++;
    }
    return count;
}

nitid_string32 nitid_string32_from_utf8(const char *utf8) {
    nitid_string32 result;
    size_t cp_count = utf8_cp_count(utf8);
    result.data = (uint32_t*)malloc((cp_count + 1) * sizeof(uint32_t));
    if (!result.data) { result.len = 0; result.cap = 0; return result; }
    size_t pos = 0;
    while (*utf8) {
        result.data[pos++] = utf8_decode_cp(&utf8);
    }
    result.data[pos] = 0;
    result.len = pos;
    result.cap = pos + 1;
    return result;
}

nitid_string32 nitid_string32_from_n(const uint32_t *s, size_t n) {
    nitid_string32 result;
    result.data = (uint32_t*)malloc((n + 1) * sizeof(uint32_t));
    if (!result.data) { result.len = 0; result.cap = 0; return result; }
    memcpy(result.data, s, n * sizeof(uint32_t));
    result.data[n] = 0;
    result.len = n;
    result.cap = n + 1;
    return result;
}

nitid_string32 nitid_string32_from_utf16(const uint16_t *s, size_t n) {
    nitid_string32 result;
    size_t cp_count = utf16_cp_count(s, n);
    result.data = (uint32_t*)malloc((cp_count + 1) * sizeof(uint32_t));
    if (!result.data) { result.len = 0; result.cap = 0; return result; }
    size_t pos = 0;
    for (size_t i = 0; i < n; i++) {
        uint32_t cp;
        if (s[i] >= 0xD800 && s[i] <= 0xDBFF) {
            if (i + 1 < n && s[i + 1] >= 0xDC00 && s[i + 1] <= 0xDFFF) {
                cp = ((uint32_t)(s[i] - 0xD800) << 10) | (s[i + 1] - 0xDC00) | 0x10000;
                i++;
            } else {
                cp = 0xFFFD;
            }
        } else if (s[i] >= 0xDC00 && s[i] <= 0xDFFF) {
            cp = 0xFFFD;
        } else {
            cp = s[i];
        }
        result.data[pos++] = cp;
    }
    result.data[pos] = 0;
    result.len = pos;
    result.cap = pos + 1;
    return result;
}

nitid_string32 nitid_string32_clone(const nitid_string32 *s) {
    return nitid_string32_from_n(s->data, s->len);
}

void nitid_string32_free(nitid_string32 *s) {
    if (s->data) { free(s->data); s->data = NULL; }
    s->len = 0;
    s->cap = 0;
}

uint32_t nitid_string32_at(const nitid_string32 *s, size_t index) {
    if (index < s->len) return s->data[index];
    return 0;
}

size_t nitid_string32_len(const nitid_string32 *s) {
    return s->len;
}

nitid_string nitid_string32_to_utf8(const nitid_string32 *s) {
    size_t max_bytes = s->len * 4;
    char *buf = (char*)malloc(max_bytes + 1);
    if (!buf) { nitid_string empty = {NULL, 0, 0}; return empty; }
    char *p = buf;
    for (size_t i = 0; i < s->len; i++) {
        utf8_encode_cp(s->data[i], &p);
    }
    *p = '\0';
    nitid_string result;
    result.data = buf;
    result.len = (size_t)(p - buf);
    result.cap = result.len + 1;
    return result;
}

uint16_t* nitid_string32_to_utf16(const nitid_string32 *s, size_t *out_len) {
    size_t units = 0;
    for (size_t i = 0; i < s->len; i++) {
        units += (s->data[i] >= 0x10000) ? 2 : 1;
    }
    uint16_t *buf = (uint16_t*)malloc((units + 1) * sizeof(uint16_t));
    if (!buf) { *out_len = 0; return NULL; }
    size_t pos = 0;
    for (size_t i = 0; i < s->len; i++) {
        uint32_t cp = s->data[i];
        if (cp >= 0x10000) {
            cp -= 0x10000;
            buf[pos++] = (uint16_t)(0xD800 | (cp >> 10));
            buf[pos++] = (uint16_t)(0xDC00 | (cp & 0x3FF));
        } else {
            buf[pos++] = (uint16_t)cp;
        }
    }
    buf[pos] = 0;
    *out_len = pos;
    return buf;
}

uint32_t nitid_string32_at_cp(const nitid_string32 *s, int64_t index) {
    if (index < 0) index = (int64_t)s->len + index;
    if (index < 0 || (size_t)index >= s->len) return 0;
    return s->data[index];
}

nitid_string32 nitid_string32_concat(const nitid_string32 *a, const nitid_string32 *b) {
    nitid_string32 result;
    result.len = a->len + b->len;
    result.cap = result.len + 1;
    result.data = (uint32_t*)malloc(result.cap * sizeof(uint32_t));
    if (result.data) {
        memcpy(result.data, a->data, a->len * sizeof(uint32_t));
        memcpy(result.data + a->len, b->data, b->len * sizeof(uint32_t));
        result.data[result.len] = 0;
    } else {
        result.len = 0;
        result.cap = 0;
    }
    return result;
}

bool nitid_string32_eq(const nitid_string32 *a, const nitid_string32 *b) {
    if (a->len != b->len) return false;
    return memcmp(a->data, b->data, a->len * sizeof(uint32_t)) == 0;
}

bool nitid_string32_ne(const nitid_string32 *a, const nitid_string32 *b) {
    return !nitid_string32_eq(a, b);
}

static int nitid_string32_cmp(const nitid_string32 *a, const nitid_string32 *b) {
    size_t min = a->len < b->len ? a->len : b->len;
    for (size_t i = 0; i < min; i++) {
        if (a->data[i] != b->data[i]) return a->data[i] < b->data[i] ? -1 : 1;
    }
    if (a->len < b->len) return -1;
    if (a->len > b->len) return 1;
    return 0;
}

bool nitid_string32_lt(const nitid_string32 *a, const nitid_string32 *b) {
    return nitid_string32_cmp(a, b) < 0;
}

bool nitid_string32_le(const nitid_string32 *a, const nitid_string32 *b) {
    return nitid_string32_cmp(a, b) <= 0;
}

bool nitid_string32_gt(const nitid_string32 *a, const nitid_string32 *b) {
    return nitid_string32_cmp(a, b) > 0;
}

bool nitid_string32_ge(const nitid_string32 *a, const nitid_string32 *b) {
    return nitid_string32_cmp(a, b) >= 0;
}
