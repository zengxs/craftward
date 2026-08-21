cmake_minimum_required(VERSION 3.22)

if(POLICY CMP0144)
    cmake_policy(SET CMP0144 NEW)
endif()

set(CMAKE_OSX_DEPLOYMENT_TARGET "13.0" CACHE STRING
    "Minimum supported macOS version"
)

include("${CMAKE_CURRENT_LIST_DIR}/cmake/version.cmake")
project(Craftward VERSION "${CRAFTWARD_VERSION}" LANGUAGES C CXX)

option(BUILD_TESTING "Build the Craftward test suite" OFF)
include(CTest)

if(APPLE)
    enable_language(OBJCXX)
    set(CMAKE_OBJCXX_STANDARD 20)
    set(CMAKE_OBJCXX_STANDARD_REQUIRED ON)
    set(CMAKE_OBJCXX_EXTENSIONS OFF)

    set(CRAFTWARD_BUNDLE_IDENTIFIER "app.craftward.macos.Craftward")
    set(CRAFTWARD_ENTITLEMENTS_FILE
        "${CMAKE_CURRENT_SOURCE_DIR}/macos/Entitlements.plist"
    )
    set(CRAFTWARD_APP_ICON_FILE
        "${CMAKE_CURRENT_SOURCE_DIR}/macos/Ward.icns"
    )
    set(CRAFTWARD_LILEX_FONT_FILES
        "${CMAKE_CURRENT_SOURCE_DIR}/assets/fonts/lilex/Lilex-Thin.ttf"
        "${CMAKE_CURRENT_SOURCE_DIR}/assets/fonts/lilex/Lilex-ThinItalic.ttf"
        "${CMAKE_CURRENT_SOURCE_DIR}/assets/fonts/lilex/Lilex-ExtraLight.ttf"
        "${CMAKE_CURRENT_SOURCE_DIR}/assets/fonts/lilex/Lilex-ExtraLightItalic.ttf"
        "${CMAKE_CURRENT_SOURCE_DIR}/assets/fonts/lilex/Lilex-Light.ttf"
        "${CMAKE_CURRENT_SOURCE_DIR}/assets/fonts/lilex/Lilex-LightItalic.ttf"
        "${CMAKE_CURRENT_SOURCE_DIR}/assets/fonts/lilex/Lilex-Regular.ttf"
        "${CMAKE_CURRENT_SOURCE_DIR}/assets/fonts/lilex/Lilex-Italic.ttf"
        "${CMAKE_CURRENT_SOURCE_DIR}/assets/fonts/lilex/Lilex-Medium.ttf"
        "${CMAKE_CURRENT_SOURCE_DIR}/assets/fonts/lilex/Lilex-MediumItalic.ttf"
        "${CMAKE_CURRENT_SOURCE_DIR}/assets/fonts/lilex/Lilex-SemiBold.ttf"
        "${CMAKE_CURRENT_SOURCE_DIR}/assets/fonts/lilex/Lilex-SemiBoldItalic.ttf"
        "${CMAKE_CURRENT_SOURCE_DIR}/assets/fonts/lilex/Lilex-Bold.ttf"
        "${CMAKE_CURRENT_SOURCE_DIR}/assets/fonts/lilex/Lilex-BoldItalic.ttf"
    )
    find_library(CRAFTWARD_APPKIT_FRAMEWORK AppKit REQUIRED)
    find_library(CRAFTWARD_COCOA_FRAMEWORK Cocoa REQUIRED)
    find_library(CRAFTWARD_CORE_TEXT_FRAMEWORK CoreText REQUIRED)
    find_library(CRAFTWARD_FOUNDATION_FRAMEWORK Foundation REQUIRED)
    find_library(CRAFTWARD_QUARTZ_CORE_FRAMEWORK QuartzCore REQUIRED)
    find_library(CRAFTWARD_VIRTUALIZATION_FRAMEWORK Virtualization REQUIRED)
    find_program(CRAFTWARD_CODESIGN_EXECUTABLE codesign REQUIRED)
    set(CRAFTWARD_SIGN_IDENTITY "-" CACHE STRING
        "Code-signing identity used for macOS deployment"
    )
    if(CRAFTWARD_SIGN_IDENTITY STREQUAL "")
        message(FATAL_ERROR "CRAFTWARD_SIGN_IDENTITY must not be empty.")
    endif()
else()
    message(FATAL_ERROR "Craftward currently supports macOS only.")
endif()

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF)

find_package(Qt6 6.11 REQUIRED COMPONENTS LinguistTools Protobuf Qml Quick QuickControls2 QuickDialogs2 Svg)
if(BUILD_TESTING)
    find_package(Qt6 6.11 REQUIRED COMPONENTS Test)
endif()

qt_standard_project_setup(
    REQUIRES 6.11
    I18N_SOURCE_LANGUAGE en
    I18N_TRANSLATED_LANGUAGES en zh_CN
)

set(QT_QML_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/qml")

set(CRAFTWARD_LEGAL_QRC
    "${CMAKE_BINARY_DIR}/generated/legal/legal.qrc"
    CACHE FILEPATH "Generated Craftward legal resource collection"
)
if(NOT EXISTS "${CRAFTWARD_LEGAL_QRC}")
    message(FATAL_ERROR
        "Generated legal resources were not found. "
        "Run `task xtask:licenses:generate OUTPUT_DIR=${CMAKE_BINARY_DIR}/generated/legal` "
        "before configuring the application."
    )
endif()

add_library(Ward::Core STATIC IMPORTED GLOBAL)
set(CRAFTWARD_WARD_CORE_INCLUDE_DIR
    "${CMAKE_CURRENT_SOURCE_DIR}/../core/ward-core/include"
)
set_target_properties(Ward::Core PROPERTIES
    IMPORTED_LOCATION
        "${CMAKE_CURRENT_SOURCE_DIR}/../core/dist/${CMAKE_BUILD_TYPE}/libward_core.a"
    INTERFACE_INCLUDE_DIRECTORIES
        "${CRAFTWARD_WARD_CORE_INCLUDE_DIR}"
)

set(CRAFTWARD_COMMIT_HASH "" CACHE STRING "Source commit for the Craftward application")

if(CRAFTWARD_COMMIT_HASH STREQUAL "")
    find_package(Git QUIET)
    if(Git_FOUND)
        execute_process(
            COMMAND "${GIT_EXECUTABLE}" rev-parse --short=7 HEAD
            WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/.."
            RESULT_VARIABLE craftward_git_result
            OUTPUT_VARIABLE craftward_detected_commit_hash
            OUTPUT_STRIP_TRAILING_WHITESPACE
            ERROR_QUIET
        )
        if(craftward_git_result EQUAL 0)
            set(CRAFTWARD_COMMIT_HASH "${craftward_detected_commit_hash}")
        endif()
    endif()
endif()

add_subdirectory(third_party)
add_subdirectory(src/localization)
add_subdirectory(src/ward)
add_subdirectory(qml/Craftward/Components)
add_subdirectory(qml/Craftward/Editor)
add_subdirectory(qml/Craftward/Features/Legal)
add_subdirectory(qml/Craftward/Features/Realm)
add_subdirectory(qml/Craftward/Pages)
add_subdirectory(qml/Craftward/App)
# Assets attach resources directly to the Craftward executable.
add_subdirectory(assets)

set(craftward_translation_consumers Craftward)
if(BUILD_TESTING)
    list(APPEND craftward_translation_consumers
        CraftwardCodexHistoryControllerTest
        CraftwardLocalizationControllerTest
        CraftwardRealmControllerTest
    )
endif()

qt_add_translations(
    TARGETS ${craftward_translation_consumers}
    SOURCE_TARGETS
        Craftward
        CraftwardCodex
        CraftwardComponents
        CraftwardFeaturesLegal
        CraftwardFeaturesRealm
        CraftwardLocalization
        CraftwardPages
        CraftwardRealm
    TS_FILES
        "${CMAKE_CURRENT_SOURCE_DIR}/translations/craftward_en.ts"
        "${CMAKE_CURRENT_SOURCE_DIR}/translations/craftward_zh_CN.ts"
    RESOURCE_PREFIX /i18n
    LUPDATE_OPTIONS -warnings-are-errors
    LRELEASE_OPTIONS -fail-on-invalid -fail-on-unfinished
    MERGE_QT_TRANSLATIONS
)
