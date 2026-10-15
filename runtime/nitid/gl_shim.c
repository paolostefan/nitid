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

// With a VBO bound, glVertexAttribPointer's last argument is a byte OFFSET into
// that buffer, not a host pointer. Nitid cannot spell an int->pointer cast, so
// this helper absorbs it (the same seam as nitid_shader_source below).
void nitid_vertex_attrib_offset(GLuint index, GLint size, GLenum type,
                                GLboolean normalized, GLsizei stride, GLintptr offset) {
  glad_glVertexAttribPointer(index, size, type, normalized, stride, (const void *)offset);
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

#undef glCompileShader
void glCompileShader(GLuint shader) { glad_glCompileShader(shader); }

#undef glGetShaderiv
void glGetShaderiv(GLuint shader, GLenum pname, GLint *params) {
    glad_glGetShaderiv(shader, pname, params);
}

#undef glCreateProgram
GLuint glCreateProgram(void) { return glad_glCreateProgram(); }

#undef glAttachShader
void glAttachShader(GLuint program, GLuint shader) { glad_glAttachShader(program, shader); }

#undef glBindAttribLocation
void glBindAttribLocation(GLuint program, GLuint index, const GLchar *name) {
    glad_glBindAttribLocation(program, index, name);
}

#undef glLinkProgram
void glLinkProgram(GLuint program) { glad_glLinkProgram(program); }

#undef glGetProgramiv
void glGetProgramiv(GLuint program, GLenum pname, GLint *params) {
    glad_glGetProgramiv(program, pname, params);
}

#undef glUseProgram
void glUseProgram(GLuint program) { glad_glUseProgram(program); }

#undef glDeleteShader
void glDeleteShader(GLuint shader) { glad_glDeleteShader(shader); }

#undef glDeleteProgram
void glDeleteProgram(GLuint program) { glad_glDeleteProgram(program); }

#undef glGetUniformLocation
GLint glGetUniformLocation(GLuint program, const GLchar *name) {
    return glad_glGetUniformLocation(program, name);
}
#undef glUniform1f
void glUniform1f(GLint location, GLfloat v0) { glad_glUniform1f(location, v0); }

#undef glEnableVertexAttribArray
void glEnableVertexAttribArray(GLuint index) { glad_glEnableVertexAttribArray(index); }


// glShaderSource takes const char ** - Nitid can't spell that yet
void nitid_shader_source(GLuint shader, const char *src) {
  glad_glShaderSource(shader, 1, &src, NULL);
}