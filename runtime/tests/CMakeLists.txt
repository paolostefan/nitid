cmake_minimum_required(VERSION 3.10)
project(nitid_runtime_tests C)

set(CMAKE_C_STANDARD 17)
set(CMAKE_C_STANDARD_REQUIRED ON)

add_executable(nitid_runtime_tests
    test_main.c
    test_nitid_array.c
    test_nitid_string.c
    test_nitid_string16.c
    test_nitid_string32.c
)

target_include_directories(nitid_runtime_tests PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/..)
target_link_libraries(nitid_runtime_tests PRIVATE nitid_runtime)
