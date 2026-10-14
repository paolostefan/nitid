cmake_minimum_required(VERSION 3.10)
project(nitid_runtime C)

set(CMAKE_C_STANDARD 17)
set(CMAKE_C_STANDARD_REQUIRED ON)

option(BUILD_TESTS "Build runtime tests" OFF)

add_library(nitid_runtime STATIC
    nitid/array.c
    nitid/string.c
    nitid/string16.c
    nitid/string32.c
    3rd_party/gl.c
    nitid/gl_shim.c
)

target_include_directories(nitid_runtime PUBLIC
                           ${CMAKE_CURRENT_SOURCE_DIR}
                           ${CMAKE_CURRENT_SOURCE_DIR}/3rd_party
)

if(BUILD_TESTS)
    enable_testing()
    add_subdirectory(tests)
endif()
