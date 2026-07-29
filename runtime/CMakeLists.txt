cmake_minimum_required(VERSION 3.10)
project(nitid_runtime C)

set(CMAKE_C_STANDARD 17)
set(CMAKE_C_STANDARD_REQUIRED ON)

option(BUILD_TESTS "Build runtime tests" OFF)

add_library(nitid_runtime STATIC
    nitid_array.c
    nitid_string.c
    nitid_string16.c
    nitid_string32.c
)

target_include_directories(nitid_runtime PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})

if(BUILD_TESTS)
    enable_testing()
    add_subdirectory(tests)
endif()
