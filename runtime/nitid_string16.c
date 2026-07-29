#include "nitid_string16.h"

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

static size_t utf8_to_utf16_count(const char *s) {
    size_t n = 0;
    while (*s) {
        uint32_t cp = utf8_decode_cp(&s);
        n += (cp >= 0x10000) ? 2 : 1;
    }
    return n;
}

static size_t utf8_to_cp_count(const char *s) {
    size_t n = 0;
    while (*s) { utf8_decode_cp(&s); n++; }
    return n;
}

static size_t utf16_cp_count(const uint16_t *s, size_t n) {
    size_t count = 0;
    for (size_t i = 0; i < n; i++) {
        if (s[i] >= 0xD800 && s[i] <= 0xDBFF) {
            if (i + 1 < n && s[i + 1] >= 0xDC00 && s[i + 1] <= 0xDFFF) {
                i++;
            }
        }
        count++;
    }
    return count;
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

nitid_string16 nitid_string16_from_utf8(const char *utf8) {
    nitid_string16 result;
    size_t units = utf8_to_utf16_count(utf8);
    result.data = (uint16_t*)malloc((units + 1) * sizeof(uint16_t));
    if (!result.data) { result.len = 0; result.cap = 0; return result; }
    size_t pos = 0;
    while (*utf8) {
        uint32_t cp = utf8_decode_cp(&utf8);
        if (cp >= 0x10000) {
            cp -= 0x10000;
            result.data[pos++] = (uint16_t)(0xD800 | (cp >> 10));
            result.data[pos++] = (uint16_t)(0xDC00 | (cp & 0x3FF));
        } else {
            result.data[pos++] = (uint16_t)cp;
        }
    }
    result.data[pos] = 0;
    result.len = pos;
    result.cap = pos + 1;
    return result;
}

nitid_string16 nitid_string16_from_n(const uint16_t *s, size_t n) {
    nitid_string16 result;
    result.data = (uint16_t*)malloc((n + 1) * sizeof(uint16_t));
    if (!result.data) { result.len = 0; result.cap = 0; return result; }
    memcpy(result.data, s, n * sizeof(uint16_t));
    result.data[n] = 0;
    result.len = n;
    result.cap = n + 1;
    return result;
}

nitid_string16 nitid_string16_from_utf32(const uint32_t *s, size_t n) {
    nitid_string16 result;
    size_t units = 0;
    for (size_t i = 0; i < n; i++) {
        units += (s[i] >= 0x10000) ? 2 : 1;
    }
    result.data = (uint16_t*)malloc((units + 1) * sizeof(uint16_t));
    if (!result.data) { result.len = 0; result.cap = 0; return result; }
    size_t pos = 0;
    for (size_t i = 0; i < n; i++) {
        uint32_t cp = s[i];
        if (cp >= 0x10000) {
            cp -= 0x10000;
            result.data[pos++] = (uint16_t)(0xD800 | (cp >> 10));
            result.data[pos++] = (uint16_t)(0xDC00 | (cp & 0x3FF));
        } else {
            result.data[pos++] = (uint16_t)cp;
        }
    }
    result.data[pos] = 0;
    result.len = pos;
    result.cap = pos + 1;
    return result;
}

nitid_string16 nitid_string16_clone(const nitid_string16 *s) {
    return nitid_string16_from_n(s->data, s->len);
}

void nitid_string16_free(nitid_string16 *s) {
    if (s->data) { free(s->data); s->data = NULL; }
    s->len = 0;
    s->cap = 0;
}

uint32_t nitid_string16_at(const nitid_string16 *s, size_t index) {
    size_t count = 0;
    for (size_t i = 0; i < s->len; i++) {
        if (s->data[i] >= 0xD800 && s->data[i] <= 0xDBFF) {
            if (count == index) {
                if (i + 1 < s->len && s->data[i + 1] >= 0xDC00 && s->data[i + 1] <= 0xDFFF) {
                    uint32_t hi = s->data[i] - 0xD800;
                    uint32_t lo = s->data[i + 1] - 0xDC00;
                    return (hi << 10) | lo | 0x10000;
                }
                return 0xFFFD;
            }
            i++;
            count++;
        } else if (s->data[i] >= 0xDC00 && s->data[i] <= 0xDFFF) {
            if (count == index) return 0xFFFD;
            count++;
        } else {
            if (count == index) return s->data[i];
            count++;
        }
    }
    return 0;
}

size_t nitid_string16_len(const nitid_string16 *s) {
    return utf16_cp_count(s->data, s->len);
}

nitid_string nitid_string16_to_utf8(const nitid_string16 *s) {
    size_t cp_count = utf16_cp_count(s->data, s->len);
    size_t max_bytes = cp_count * 4;
    char *buf = (char*)malloc(max_bytes + 1);
    if (!buf) { nitid_string empty = {NULL, 0, 0}; return empty; }
    char *p = buf;
    for (size_t i = 0; i < s->len; i++) {
        uint32_t cp;
        if (s->data[i] >= 0xD800 && s->data[i] <= 0xDBFF) {
            if (i + 1 < s->len && s->data[i + 1] >= 0xDC00 && s->data[i + 1] <= 0xDFFF) {
                uint32_t hi = s->data[i] - 0xD800;
                uint32_t lo = s->data[i + 1] - 0xDC00;
                cp = (hi << 10) | lo | 0x10000;
                i++;
            } else {
                cp = 0xFFFD;
            }
        } else if (s->data[i] >= 0xDC00 && s->data[i] <= 0xDFFF) {
            cp = 0xFFFD;
        } else {
            cp = s->data[i];
        }
        utf8_encode_cp(cp, &p);
    }
    *p = '\0';
    nitid_string result;
    result.data = buf;
    result.len = (size_t)(p - buf);
    result.cap = result.len + 1;
    return result;
}

uint32_t* nitid_string16_to_utf32(const nitid_string16 *s, size_t *out_len) {
    size_t cp_count = utf16_cp_count(s->data, s->len);
    uint32_t *buf = (uint32_t*)malloc(cp_count * sizeof(uint32_t));
    if (!buf) { *out_len = 0; return NULL; }
    size_t pos = 0;
    for (size_t i = 0; i < s->len; i++) {
        uint32_t cp;
        if (s->data[i] >= 0xD800 && s->data[i] <= 0xDBFF) {
            if (i + 1 < s->len && s->data[i + 1] >= 0xDC00 && s->data[i + 1] <= 0xDFFF) {
                uint32_t hi = s->data[i] - 0xD800;
                uint32_t lo = s->data[i + 1] - 0xDC00;
                cp = (hi << 10) | lo | 0x10000;
                i++;
            } else {
                cp = 0xFFFD;
            }
        } else if (s->data[i] >= 0xDC00 && s->data[i] <= 0xDFFF) {
            cp = 0xFFFD;
        } else {
            cp = s->data[i];
        }
        buf[pos++] = cp;
    }
    *out_len = pos;
    return buf;
}

uint32_t nitid_string16_at_cp(const nitid_string16 *s, int64_t index) {
    if (index < 0) {
        size_t cp_count = nitid_string16_len(s);
        index = (int64_t)cp_count + index;
    }
    if (index < 0) return 0;
    return nitid_string16_at(s, (size_t)index);
}

nitid_string16 nitid_string16_concat(const nitid_string16 *a, const nitid_string16 *b) {
    nitid_string16 result;
    result.len = a->len + b->len;
    result.cap = result.len + 1;
    result.data = (uint16_t*)malloc(result.cap * sizeof(uint16_t));
    if (result.data) {
        memcpy(result.data, a->data, a->len * sizeof(uint16_t));
        memcpy(result.data + a->len, b->data, b->len * sizeof(uint16_t));
        result.data[result.len] = 0;
    } else {
        result.len = 0;
        result.cap = 0;
    }
    return result;
}

static int nitid_string16_cmp(const nitid_string16 *a, const nitid_string16 *b) {
    size_t i = 0, j = 0;
    while (i < a->len && j < b->len) {
        uint32_t ca;
        if (a->data[i] >= 0xD800 && a->data[i] <= 0xDBFF) {
            if (i + 1 < a->len && a->data[i + 1] >= 0xDC00 && a->data[i + 1] <= 0xDFFF) {
                ca = ((uint32_t)(a->data[i] - 0xD800) << 10) | (a->data[i + 1] - 0xDC00) | 0x10000;
                i += 2;
            } else { ca = 0xFFFD; i++; }
        } else if (a->data[i] >= 0xDC00 && a->data[i] <= 0xDFFF) { ca = 0xFFFD; i++; }
        else { ca = a->data[i]; i++; }

        uint32_t cb;
        if (b->data[j] >= 0xD800 && b->data[j] <= 0xDBFF) {
            if (j + 1 < b->len && b->data[j + 1] >= 0xDC00 && b->data[j + 1] <= 0xDFFF) {
                cb = ((uint32_t)(b->data[j] - 0xD800) << 10) | (b->data[j + 1] - 0xDC00) | 0x10000;
                j += 2;
            } else { cb = 0xFFFD; j++; }
        } else if (b->data[j] >= 0xDC00 && b->data[j] <= 0xDFFF) { cb = 0xFFFD; j++; }
        else { cb = b->data[j]; j++; }

        if (ca != cb) return ca < cb ? -1 : 1;
    }
    if (i < a->len) return 1;
    if (j < b->len) return -1;
    return 0;
}

bool nitid_string16_eq(const nitid_string16 *a, const nitid_string16 *b) {
    if (a->len != b->len) return false;
    return memcmp(a->data, b->data, a->len * sizeof(uint16_t)) == 0;
}

bool nitid_string16_ne(const nitid_string16 *a, const nitid_string16 *b) {
    return !nitid_string16_eq(a, b);
}

bool nitid_string16_lt(const nitid_string16 *a, const nitid_string16 *b) {
    return nitid_string16_cmp(a, b) < 0;
}

bool nitid_string16_le(const nitid_string16 *a, const nitid_string16 *b) {
    return nitid_string16_cmp(a, b) <= 0;
}

bool nitid_string16_gt(const nitid_string16 *a, const nitid_string16 *b) {
    return nitid_string16_cmp(a, b) > 0;
}

bool nitid_string16_ge(const nitid_string16 *a, const nitid_string16 *b) {
    return nitid_string16_cmp(a, b) >= 0;
}
