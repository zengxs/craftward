include_guard(GLOBAL)
include("${CMAKE_CURRENT_LIST_DIR}/contour-meson.cmake")

function(craftward_add_contour_fonts source_root)
    set(CMAKE_AUTOMOC OFF)
    set(CMAKE_AUTORCC OFF)
    set(CMAKE_AUTOUIC OFF)
    set(CMAKE_CXX_STANDARD 20)
    set(CMAKE_POLICY_VERSION_MINIMUM 3.10)
    set(CMAKE_POLICY_DEFAULT_CMP0077 NEW)
    set(BUILD_SHARED_LIBS OFF)
    set(SKIP_INSTALL_ALL ON)
    set(CMAKE_FIND_FRAMEWORK LAST)
    set(CMAKE_IGNORE_PREFIX_PATH /opt/homebrew /usr/local)
    set(font_binary "${CMAKE_CURRENT_BINARY_DIR}/contour-fonts")
    find_package(ZLIB REQUIRED)

    set(PNG_SHARED OFF)
    set(PNG_STATIC ON)
    set(PNG_TESTS OFF)
    set(PNG_TOOLS OFF)
    set(PNG_DEBUG_POSTFIX "")
    add_subdirectory("${source_root}/libpng" "${font_binary}/libpng" EXCLUDE_FROM_ALL)
    # Upstream selects one CPU at configure time. pngpriv.h can select SSE/NEON
    # per compiler slice instead, provided both guarded implementations are present.
    if("arm64" IN_LIST CMAKE_OSX_ARCHITECTURES AND "x86_64" IN_LIST CMAKE_OSX_ARCHITECTURES)
        get_directory_property(png_definitions DIRECTORY "${source_root}/libpng" COMPILE_DEFINITIONS)
        list(FILTER png_definitions EXCLUDE REGEX "^PNG_(ARM_NEON|INTEL_SSE)_")
        set_property(DIRECTORY "${source_root}/libpng" PROPERTY COMPILE_DEFINITIONS "${png_definitions}")
        get_target_property(png_sources png_static SOURCES)
        list(FILTER png_sources EXCLUDE REGEX "(^|/)(arm|intel)/")
        set_property(TARGET png_static PROPERTY SOURCES "${png_sources}")
        target_sources(png_static PRIVATE
            "${source_root}/libpng/arm/arm_init.c"
            "${source_root}/libpng/arm/filter_neon_intrinsics.c"
            "${source_root}/libpng/arm/palette_neon_intrinsics.c"
            "${source_root}/libpng/intel/intel_init.c"
            "${source_root}/libpng/intel/filter_sse2_intrinsics.c"
        )
        target_compile_definitions(png_static PRIVATE PNG_INTEL_SSE)
    endif()
    add_library(PNG::PNG ALIAS png_static)
    # FreeType uses FindPNG even when a source-built target already exists.
    set(PNG_LIBRARY png_static)
    set(PNG_PNG_INCLUDE_DIR "${source_root}/libpng")
    set(FT_DISABLE_HARFBUZZ ON)
    set(FT_DISABLE_BZIP2 ON)
    set(FT_DISABLE_BROTLI ON)
    set(FT_REQUIRE_PNG ON)
    set(FT_REQUIRE_ZLIB ON)
    add_subdirectory("${source_root}/freetype" "${font_binary}/freetype" EXCLUDE_FROM_ALL)
    add_library(Freetype::Freetype ALIAS freetype)

    set(HB_HAVE_FREETYPE ON)
    set(HB_BUILD_SUBSET OFF)
    set(HB_BUILD_RASTER OFF)
    set(HB_BUILD_VECTOR OFF)
    set(HB_BUILD_GPU OFF)
    set(HB_BUILD_UTILS OFF)
    add_subdirectory("${source_root}/harfbuzz" "${font_binary}/harfbuzz" EXCLUDE_FROM_ALL)
    add_library(HarfBuzz::HarfBuzz ALIAS harfbuzz)
    # Contour uses installed-style <harfbuzz/...> includes. Keep them tied directly
    # to the submodule headers so header edits participate in normal dependency tracking.
    file(MAKE_DIRECTORY "${font_binary}/include")
    file(CREATE_LINK "${source_root}/harfbuzz/src" "${font_binary}/include/harfbuzz" SYMBOLIC)
    target_include_directories(harfbuzz INTERFACE "${font_binary}/include")
    # HarfBuzz skips its FreeType capability probes when freetype is a local target.
    # Probe the source headers without linking a library that has not been built yet.
    include(CheckCSourceCompiles)
    set(CMAKE_TRY_COMPILE_TARGET_TYPE STATIC_LIBRARY)
    set(CMAKE_REQUIRED_INCLUDES "${font_binary}/freetype/include" "${source_root}/freetype/include")
    foreach(function IN ITEMS FT_Get_Var_Blend_Coordinates FT_Set_Var_Blend_Coordinates
            FT_Done_MM_Var FT_Get_Transform)
        string(TOUPPER "${function}" capability)
        check_c_source_compiles("#include <ft2build.h>
            #include FT_FREETYPE_H
            #include FT_MULTIPLE_MASTERS_H
            int main(void) { (void) &${function}; return 0; }" "CRAFTWARD_HB_HAVE_${capability}")
        if(CRAFTWARD_HB_HAVE_${capability})
            target_compile_definitions(harfbuzz PRIVATE "HAVE_${capability}=1")
        endif()
    endforeach()
    # Preserve the optimized font path in developer builds as well.
    foreach(target IN ITEMS png_static freetype harfbuzz)
        target_compile_options(${target} PRIVATE "$<$<CONFIG:Debug>:-O3>")
        target_compile_definitions(${target} PRIVATE "$<$<CONFIG:Debug>:NDEBUG>")
    endforeach()

    craftward_add_contour_meson("${source_root}" "${font_binary}")
endfunction()
