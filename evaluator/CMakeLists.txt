cmake_minimum_required(VERSION 3.20)

project(aoc_roi_counter LANGUAGES C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

find_package(PkgConfig REQUIRED)
pkg_check_modules(GLIB REQUIRED IMPORTED_TARGET glib-2.0)
find_path(QEMU_PLUGIN_INCLUDE_DIR qemu-plugin.h REQUIRED)

add_library(roi-counter MODULE roi-counter.c)
target_include_directories(roi-counter PRIVATE "${QEMU_PLUGIN_INCLUDE_DIR}")
target_link_libraries(roi-counter PRIVATE PkgConfig::GLIB)
target_compile_options(roi-counter PRIVATE -Wall -Wextra -Werror)

# QEMU resolves the plugin API when it loads the module. ELF linkers permit
# those unresolved symbols by default, while Darwin's linker needs this flag.
if(APPLE)
    target_link_options(roi-counter PRIVATE "LINKER:-undefined,dynamic_lookup")
endif()

set_target_properties(
    roi-counter
    PROPERTIES
        C_VISIBILITY_PRESET hidden
        LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}"
        POSITION_INDEPENDENT_CODE ON
        PREFIX ""
)
