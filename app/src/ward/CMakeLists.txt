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
    add_library(CraftwardWardCoreHeaderCSmoke OBJECT
        tests/ward_core_header_smoke.c
    )
    target_link_libraries(CraftwardWardCoreHeaderCSmoke PRIVATE Ward::Core)

    add_library(CraftwardWardCoreHeaderCxxSmoke OBJECT
        tests/ward_core_header_smoke.cpp
    )
    target_link_libraries(CraftwardWardCoreHeaderCxxSmoke PRIVATE Ward::Core)

    qt_add_executable(CraftwardCoreFfiSupportTest
        tests/coreffierrortest.cpp
    )
    add_dependencies(CraftwardCoreFfiSupportTest
        CraftwardWardCoreHeaderCSmoke
        CraftwardWardCoreHeaderCxxSmoke
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
