cmake_minimum_required(VERSION 3.16)
project(binpack_loader CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Default to Release if not set
if(NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE Release)
endif()

add_library(binpack_loader SHARED binpack_loader.cpp)

target_include_directories(binpack_loader PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})

target_compile_options(binpack_loader PRIVATE
    -O3
    -march=native
    -ffast-math
)

# BMI2 detection (x86 only) — ARM falls back to software implementation
include(CheckCXXSourceCompiles)
check_cxx_source_compiles("
    #include <immintrin.h>
    int main() { _pdep_u64(0, 0); return 0; }
" HAS_BMI2)
if(HAS_BMI2)
    target_compile_definitions(binpack_loader PRIVATE HAS_BMI2)
    message(STATUS "BMI2 detected — enabling fast popcount")
endif()

# Output the shared lib directly in build/
set_target_properties(binpack_loader PROPERTIES
    LIBRARY_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}
    RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}
)
