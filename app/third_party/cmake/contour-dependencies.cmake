include_guard(GLOBAL)

function(craftward_add_contour_dependencies source_root)
    # Dependency feature checks must describe the target slice when cross-compiling.
    if(CMAKE_OSX_ARCHITECTURES MATCHES "^(arm64|x86_64)$")
        set(CMAKE_SYSTEM_PROCESSOR "${CMAKE_OSX_ARCHITECTURES}")
    endif()
    set(CMAKE_CXX_STANDARD 23)
    set(CMAKE_POLICY_VERSION_MINIMUM 3.10)
    set(CMAKE_POLICY_DEFAULT_CMP0077 NEW)
    set(CMAKE_POLICY_DEFAULT_CMP0090 NEW)
    set(CMAKE_EXPORT_PACKAGE_REGISTRY OFF)
    set(BUILD_SHARED_LIBS OFF)
    set(CMAKE_AUTOMOC OFF)
    foreach(option IN ITEMS GSL_TEST YAML_CPP_BUILD_TESTS YAML_CPP_BUILD_TOOLS
            YAML_CPP_BUILD_CONTRIB LIBUNICODE_TESTING LIBUNICODE_BENCHMARK
            LIBUNICODE_TOOLS LIBUNICODE_EXAMPLES BOXED_CPP_BUILD_TESTS
            REFLECTION_CPP_BUILD_TESTS ENABLE_TIDY)
        set(${option} OFF)
    endforeach()
    set(LIBUNICODE_UCD_BASE_DIR "${CMAKE_BINARY_DIR}/_deps/libunicode-ucd" CACHE PATH
        "Build-local Unicode character database" FORCE)
    # Let libunicode derive this path from its own Unicode version and the new cache root.
    unset(LIBUNICODE_UCD_DIR CACHE)
    foreach(name IN ITEMS GSL yaml-cpp boxed-cpp reflection-cpp libunicode)
        if(NOT EXISTS "${source_root}/${name}/CMakeLists.txt")
            message(FATAL_ERROR "Initialize the app/third_party/${name} submodule before configuring.")
        endif()
        add_subdirectory("${source_root}/${name}" "${CMAKE_CURRENT_BINARY_DIR}/${name}"
            EXCLUDE_FROM_ALL)
    endforeach()

    # libunicode selects x86 SIMD translation units from the build machine's CPU.
    # Universal builds need those units for the Intel slice and NEON for the ARM slice.
    if("x86_64" IN_LIST CMAKE_OSX_ARCHITECTURES AND "arm64" IN_LIST CMAKE_OSX_ARCHITECTURES)
        set(unicode_source "${source_root}/libunicode/src/libunicode")
        get_target_property(unicode_sources unicode SOURCES)
        list(FILTER unicode_sources EXCLUDE REGEX "(^|/)(simd_detector|scan256|scan512|convert256|convert512)\\.cpp$")
        set_property(TARGET unicode PROPERTY SOURCES "${unicode_sources}")
        set_source_files_properties("${unicode_source}/convert.cpp"
            DIRECTORY "${unicode_source}" PROPERTIES COMPILE_FLAGS "-Xarch_x86_64 -msse4.1")
        foreach(unit IN ITEMS simd_detector scan256 scan512 convert256 convert512)
            set(wrapper "${CMAKE_CURRENT_BINARY_DIR}/unicode-x86/${unit}.cpp")
            file(CONFIGURE OUTPUT "${wrapper}" CONTENT
                "#if defined(__x86_64__)\n#include \"${unicode_source}/${unit}.cpp\"\n#endif\n" @ONLY)
            target_sources(unicode PRIVATE "${wrapper}")
            if(unit MATCHES "256$")
                set_source_files_properties("${wrapper}" TARGET_DIRECTORY unicode PROPERTIES
                    COMPILE_FLAGS "-Xarch_x86_64 -mavx2")
            elseif(unit MATCHES "512$")
                set_source_files_properties("${wrapper}" TARGET_DIRECTORY unicode PROPERTIES
                    COMPILE_FLAGS "-Xarch_x86_64 -mavx512f -Xarch_x86_64 -mavx512bw")
            endif()
        endforeach()
    endif()

endfunction()
