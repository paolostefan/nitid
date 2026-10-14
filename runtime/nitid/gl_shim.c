#include <glad/gl.h>
#include <SDL3/SDL.h>

// SDL_GL_GetProcAddress is a void* (*)(const char*); GLADloadfunc is the same
// shape under another name. This cast is the one Nitid cannot spell.
int nitid_glad_load(void) {
    return gladLoadGL((GLADloadfunc) SDL_GL_GetProcAddress);
}

// The macro is not a symbol. Undefine it, then define a real callable one.
#undef glClear
void glClear(GLbitfield mask) { glad_glClear(mask); }

#undef glClearColor
void glClearColor(GLfloat r, GLfloat g, GLfloat b, GLfloat a) {
    glad_glClearColor(r, g, b, a);
}

#undef glGenBuffers
void glGenBuffers(GLsizei n, GLuint * buffers) {
  glad_glGenBuffers(n, buffers);
}

#undef glBindBuffer
void glBindBuffer(GLenum target, GLuint buffer) {
  glad_glBindBuffer(target, buffer);
}

#undef glBufferData
void glBufferData(GLenum target, GLsizeiptr size, const void * data, GLenum usage) {
  glad_glBufferData(target, size, data, usage);
}

#undef glVertexAttribPointer
void glVertexAttribPointer(GLuint index, GLint size, GLenum type, GLboolean normalized, GLsizei stride, const void * pointer) {
  glad_glVertexAttribPointer(index, size, type, normalized, stride, pointer);
}

#undef glCreateShader
GLuint glCreateShader(GLenum type) {
  return glad_glCreateShader(type);
}

#undef glShaderSource
void glShaderSource(GLuint shader, GLsizei count, const GLchar *const* string, const GLint * length) {
  glad_glShaderSource(shader, count, string, length);
}

#undef glDrawArrays
void glDrawArrays(GLenum mode, GLint first, GLsizei count) {
  glad_glDrawArrays(mode, first, count);
}