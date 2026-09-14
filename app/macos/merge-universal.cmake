cmake_minimum_required(VERSION 3.28)

foreach(required IN ITEMS ARM64_BUNDLE X86_64_BUNDLE OUTPUT_BUNDLE ENTITLEMENTS)
    if(NOT DEFINED ${required} OR "${${required}}" STREQUAL "")
        message(FATAL_ERROR "${required} is required.")
    endif()
endforeach()

execute_process(COMMAND /usr/bin/plutil -extract CFBundleExecutable raw
    "${ARM64_BUNDLE}/Contents/Info.plist"
    OUTPUT_VARIABLE executable OUTPUT_STRIP_TRAILING_WHITESPACE COMMAND_ERROR_IS_FATAL ANY)
set(binary "Contents/MacOS/${executable}")
file(REAL_PATH "${OUTPUT_BUNDLE}" output_path)
foreach(arch IN ITEMS arm64 x86_64)
    string(TOUPPER "${arch}" arch_key)
    set(bundle "${${arch_key}_BUNDLE}")
    file(REAL_PATH "${bundle}" source_path)
    cmake_path(IS_PREFIX output_path "${source_path}" NORMALIZE contains_source)
    cmake_path(IS_PREFIX source_path "${output_path}" NORMALIZE contains_output)
    if(contains_source OR contains_output)
        message(FATAL_ERROR "The output bundle must be separate from both input bundles.")
    endif()
    execute_process(COMMAND /usr/bin/lipo -archs "${bundle}/${binary}"
        OUTPUT_VARIABLE actual_arch OUTPUT_STRIP_TRAILING_WHITESPACE COMMAND_ERROR_IS_FATAL ANY)
    if(NOT actual_arch STREQUAL arch)
        message(FATAL_ERROR "Expected ${arch} in ${bundle}, found ${actual_arch}.")
    endif()
    file(GLOB_RECURSE files LIST_DIRECTORIES FALSE RELATIVE "${bundle}" "${bundle}/*")
    # All application modules are statically linked into the main executable.
    # Resources must be identical; signatures are regenerated after merging.
    list(FILTER files EXCLUDE REGEX "^Contents/_CodeSignature/")
    list(REMOVE_ITEM files "${binary}")
    set(${arch}_files "${files}")
endforeach()
if(NOT arm64_files STREQUAL x86_64_files)
    message(FATAL_ERROR "Bundle file lists differ between arm64 and x86_64.")
endif()
foreach(resource IN LISTS arm64_files)
    if(IS_SYMLINK "${ARM64_BUNDLE}/${resource}" AND IS_SYMLINK "${X86_64_BUNDLE}/${resource}")
        file(READ_SYMLINK "${ARM64_BUNDLE}/${resource}" arm_content)
        file(READ_SYMLINK "${X86_64_BUNDLE}/${resource}" intel_content)
    elseif(IS_SYMLINK "${ARM64_BUNDLE}/${resource}" OR IS_SYMLINK "${X86_64_BUNDLE}/${resource}")
        message(FATAL_ERROR "Bundle resource differs: ${resource}")
    else()
        file(SHA256 "${ARM64_BUNDLE}/${resource}" arm_content)
        file(SHA256 "${X86_64_BUNDLE}/${resource}" intel_content)
    endif()
    if(NOT arm_content STREQUAL intel_content)
        message(FATAL_ERROR "Bundle resource differs: ${resource}")
    endif()
endforeach()

file(REMOVE_RECURSE "${OUTPUT_BUNDLE}")
execute_process(COMMAND /usr/bin/ditto "${ARM64_BUNDLE}" "${OUTPUT_BUNDLE}"
    COMMAND_ERROR_IS_FATAL ANY)
execute_process(COMMAND /usr/bin/lipo -create
    "${ARM64_BUNDLE}/${binary}" "${X86_64_BUNDLE}/${binary}"
    -output "${OUTPUT_BUNDLE}/${binary}" COMMAND_ERROR_IS_FATAL ANY)
execute_process(COMMAND /usr/bin/lipo "${OUTPUT_BUNDLE}/${binary}" -verify_arch arm64 x86_64
    COMMAND_ERROR_IS_FATAL ANY)

set(identity "$ENV{CRAFTWARD_SIGN_IDENTITY}")
if(identity STREQUAL "")
    set(identity "-")
endif()
set(sign_options --force --sign "${identity}" --entitlements "${ENTITLEMENTS}" --timestamp=none)
if(NOT identity STREQUAL "-")
    list(APPEND sign_options --options runtime)
endif()
execute_process(COMMAND /usr/bin/codesign ${sign_options} "${OUTPUT_BUNDLE}"
    COMMAND_ERROR_IS_FATAL ANY)
execute_process(COMMAND /usr/bin/codesign --verify --strict "${OUTPUT_BUNDLE}"
    COMMAND_ERROR_IS_FATAL ANY)
message(STATUS "Universal2 application: ${OUTPUT_BUNDLE}")
