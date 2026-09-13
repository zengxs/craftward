include_guard(GLOBAL)
include(ExternalProject)

function(craftward_add_contour_meson source_root font_binary)
    find_program(contour_meson meson REQUIRED)
    find_program(contour_ninja ninja REQUIRED)
    find_package(PkgConfig REQUIRED)
    execute_process(COMMAND "${contour_meson}" --version
        OUTPUT_VARIABLE meson_version OUTPUT_STRIP_TRAILING_WHITESPACE COMMAND_ERROR_IS_FATAL ANY)
    if(meson_version VERSION_LESS 1.3.0)
        message(FATAL_ERROR "Contour's font dependencies require Meson 1.3.0 or later.")
    endif()
    if(IS_ABSOLUTE "${CMAKE_OSX_SYSROOT}")
        set(sdk "${CMAKE_OSX_SYSROOT}")
    else()
        set(sdk_name "${CMAKE_OSX_SYSROOT}")
        if(NOT sdk_name)
            set(sdk_name macosx)
        endif()
        execute_process(COMMAND xcrun --sdk "${sdk_name}" --show-sdk-path
            OUTPUT_VARIABLE sdk OUTPUT_STRIP_TRAILING_WHITESPACE COMMAND_ERROR_IS_FATAL ANY)
    endif()
    file(REAL_PATH "${sdk}" sdk)
    set(meson_root "${CMAKE_CURRENT_BINARY_DIR}/contour-meson")
    set(pc_directory "${meson_root}/pkgconfig")
    file(MAKE_DIRECTORY "${pc_directory}")

    # Meson consumes the source-built CMake libraries through build-local metadata.
    # FreeType's pkg-config version uses libtool numbering, not its release number.
    file(STRINGS "${source_root}/freetype/builds/unix/configure.raw" ft_version REGEX "^version_info=")
    string(REGEX MATCH "([0-9]+):([0-9]+):([0-9]+)" ft_version "${ft_version}")
    set(ft_version "Version: ${CMAKE_MATCH_1}.${CMAKE_MATCH_2}.${CMAKE_MATCH_3}")
    file(STRINGS "${source_root}/libpng/png.h" png_version REGEX "^#define PNG_LIBPNG_VER_STRING ")
    string(REGEX MATCH "[0-9]+\\.[0-9]+\\.[0-9]+" png_version "${png_version}")
    file(STRINGS "${sdk}/usr/include/zlib.h" zlib_version REGEX "^#define ZLIB_VERSION ")
    string(REGEX MATCH "[0-9]+\\.[0-9]+\\.[0-9]+" zlib_version "${zlib_version}")
    file(GENERATE OUTPUT "${pc_directory}/libpng.pc" CONTENT
        "Name: libpng\nDescription: Craftward source-built libpng\nVersion: ${png_version}\nRequires.private: zlib\nLibs: \"$<TARGET_FILE:png_static>\"\nCflags: -I\"${source_root}/libpng\" -I\"${font_binary}/libpng\"\n")
    file(GENERATE OUTPUT "${pc_directory}/freetype2.pc" CONTENT
        "Name: FreeType 2\nDescription: Craftward source-built FreeType\n${ft_version}\nRequires.private: zlib libpng\nLibs: \"$<TARGET_FILE:freetype>\"\nCflags: -I\"${font_binary}/freetype/include\" -I\"${source_root}/freetype/include\"\n")
    file(CONFIGURE OUTPUT "${pc_directory}/zlib.pc" CONTENT
        "Name: zlib\nDescription: macOS SDK zlib\nVersion: ${zlib_version}\nLibs: -lz\n" @ONLY)
    get_filename_component(ninja_directory "${contour_ninja}" DIRECTORY)
    set(architectures ${CMAKE_OSX_ARCHITECTURES})
    if(NOT architectures)
        set(architectures "${CMAKE_SYSTEM_PROCESSOR}")
    endif()
    list(GET architectures 0 header_architecture)
    foreach(arch IN LISTS architectures)
        if(arch STREQUAL "arm64")
            set(cpu_family aarch64)
        elseif(arch STREQUAL "x86_64")
            set(cpu_family x86_64)
        else()
            message(FATAL_ERROR "Unsupported Contour architecture: ${arch}")
        endif()
        set(meson_prefix "${meson_root}/${arch}/install")
        set(machine_file "${meson_root}/${arch}/macos.ini")
        file(MAKE_DIRECTORY "${meson_prefix}/include/cairo" "${meson_prefix}/include/pixman-1")
        configure_file("${CMAKE_CURRENT_FUNCTION_LIST_DIR}/contour-meson-cross.ini.in"
            "${machine_file}" @ONLY)
        set(meson_environment "${CMAKE_COMMAND}" -E env
            --unset=CFLAGS --unset=CXXFLAGS --unset=CPPFLAGS --unset=LDFLAGS
            --unset=CPATH --unset=LIBRARY_PATH --unset=PKG_CONFIG_PATH
            "PATH=${ninja_directory}:$ENV{PATH}"
            "PKG_CONFIG_LIBDIR=${pc_directory}:${meson_prefix}/lib/pkgconfig:${sdk}/usr/lib/pkgconfig"
            "MACOSX_DEPLOYMENT_TARGET=${CMAKE_OSX_DEPLOYMENT_TARGET}")
        foreach(name IN ITEMS pixman cairo)
            set(meson_source "${source_root}/${name}")
            set(meson_build "${meson_root}/${arch}/${name}")
            set(project "CraftwardContour_${name}_${arch}")
            set(prerequisites)
            set(configuration_inputs "${machine_file}" "${contour_meson}" "${PKG_CONFIG_EXECUTABLE}")
            if(name STREQUAL "pixman")
                set(library_name pixman-1)
                set(meson_options -Dtests=disabled -Ddemos=disabled -Dgtk=disabled)
                set(headers pixman.h pixman-version.h)
                set(include_directory pixman-1)
            else()
                set(library_name cairo)
                set(meson_options -Dtests=disabled -Dgtk_doc=false -Dglib=disabled
                    -Dfontconfig=disabled -Dfreetype=enabled -Dpng=enabled
                    -Dxlib=disabled -Dxcb=disabled -Dquartz=disabled -Dtee=disabled
                    -Dspectre=disabled -Dsymbol-lookup=disabled)
                set(prerequisites png_static freetype "CraftwardContour_pixman_${arch}")
                list(APPEND configuration_inputs
                    "${pc_directory}/libpng.pc" "${pc_directory}/freetype2.pc" "${pc_directory}/zlib.pc")
                set(headers cairo.h cairo-ft.h cairo-features.h cairo-version.h cairo-deprecated.h)
                set(include_directory cairo)
            endif()
            set(installed_headers ${headers})
            list(TRANSFORM installed_headers PREPEND "${meson_prefix}/include/${include_directory}/")
            set(archive "${meson_prefix}/lib/lib${library_name}.a")
            list(APPEND ${name}_archives "${archive}")
            set(setup_script "${meson_root}/${arch}/${name}-setup.cmake")
            configure_file("${CMAKE_CURRENT_FUNCTION_LIST_DIR}/contour-meson-setup.cmake.in"
                "${setup_script}" @ONLY)
            ExternalProject_Add(${project}
                SOURCE_DIR "${meson_source}" BINARY_DIR "${meson_build}"
                PREFIX "${meson_root}/${arch}/${name}-steps"
                DOWNLOAD_COMMAND "" UPDATE_COMMAND "" PATCH_COMMAND ""
                CONFIGURE_COMMAND ${meson_environment} "${CMAKE_COMMAND}" -P "${setup_script}"
                BUILD_COMMAND ${meson_environment} "${contour_ninja}" -C "${meson_build}" -j 4
                INSTALL_COMMAND ${meson_environment} "${contour_ninja}" -C "${meson_build}" install
                INSTALL_BYPRODUCTS "${archive}" ${installed_headers}
                DEPENDS ${prerequisites}
                EXCLUDE_FROM_ALL TRUE
            )
            # Local source edits must reach Ninja even without a submodule revision change.
            file(GLOB_RECURSE source_inputs CONFIGURE_DEPENDS LIST_DIRECTORIES FALSE "${meson_source}/*")
            ExternalProject_Add_StepDependencies(${project} configure "${setup_script}" ${configuration_inputs})
            ExternalProject_Add_StepDependencies(${project} build ${source_inputs})
            if(name STREQUAL "cairo")
                ExternalProject_Add_StepDependencies(${project} build
                    "$<TARGET_FILE:png_static>" "$<TARGET_FILE:freetype>"
                    "${meson_prefix}/lib/libpixman-1.a")
            endif()
        endforeach()
    endforeach()
    foreach(name IN ITEMS pixman cairo)
        if(name STREQUAL "pixman")
            set(library_name pixman-1)
        else()
            set(library_name cairo)
        endif()
        set(archive "${meson_root}/lib/lib${library_name}.a")
        add_custom_command(OUTPUT "${archive}"
            COMMAND "${CMAKE_COMMAND}" -E make_directory "${meson_root}/lib"
            COMMAND /usr/bin/lipo -create ${${name}_archives} -output "${archive}"
            DEPENDS ${${name}_archives}
            COMMENT "Combining ${name} architecture slices" VERBATIM)
        add_custom_target(CraftwardContour_${name}_archive DEPENDS "${archive}")
        add_library(CraftwardContour_${name} STATIC IMPORTED GLOBAL)
        set_target_properties(CraftwardContour_${name} PROPERTIES
            IMPORTED_LOCATION "${archive}"
            INTERFACE_INCLUDE_DIRECTORIES
                "${meson_root}/${header_architecture}/install/include;${meson_root}/${header_architecture}/install/include/${library_name}")
        add_dependencies(CraftwardContour_${name} CraftwardContour_${name}_archive)
    endforeach()
    target_link_libraries(CraftwardContour_cairo INTERFACE freetype png_static CraftwardContour_pixman)
    add_library(Cairo::Cairo ALIAS CraftwardContour_cairo)
endfunction()
