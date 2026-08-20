add_library(CraftwardCoreFfiSupport STATIC
    coreffierror.cpp
    coreffierror.h
)

target_include_directories(CraftwardCoreFfiSupport
    PUBLIC
        "$<BUILD_INTERFACE:${PROJECT_SOURCE_DIR}/src>"
)

target_link_libraries(CraftwardCoreFfiSupport
    PUBLIC
        Qt6::Core
    PRIVATE
        Ward::Core
)

if(BUILD_TESTING)
    qt_add_executable(CraftwardCoreFfiSupportTest
        tests/coreffierrortest.cpp
    )
    target_link_libraries(CraftwardCoreFfiSupportTest
        PRIVATE
            CraftwardCoreFfiSupport
            Qt6::Test
            Ward::Core
            "${CRAFTWARD_VIRTUALIZATION_FRAMEWORK}"
    )
    add_test(
        NAME CraftwardCoreFfiSupportTest
        COMMAND CraftwardCoreFfiSupportTest
    )
endif()

add_subdirectory(codex)
add_subdirectory(realm)
