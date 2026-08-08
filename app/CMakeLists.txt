cmake_minimum_required(VERSION 3.22)

project(Craftward VERSION 0.1.0 LANGUAGES CXX)

if(APPLE)
    set(CRAFTWARD_BUNDLE_IDENTIFIER "app.craftward.macos.Craftward")
    set(CRAFTWARD_ENTITLEMENTS_FILE
        "${CMAKE_CURRENT_SOURCE_DIR}/macos/Entitlements.plist"
    )
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

find_package(Qt6 6.11 REQUIRED COMPONENTS Qml Quick QuickControls2)

qt_standard_project_setup(REQUIRES 6.11)

set(QT_QML_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/qml")

set(CRAFTWARD_BUILD_NUMBER "1" CACHE STRING "Build number for the Craftward application")
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

add_subdirectory(qml/Craftward/Components)
add_subdirectory(qml/Craftward/Features/Legal)
add_subdirectory(qml/Craftward/Pages)
add_subdirectory(qml/Craftward/App)
# Assets attach resources directly to the Craftward executable.
add_subdirectory(assets)
