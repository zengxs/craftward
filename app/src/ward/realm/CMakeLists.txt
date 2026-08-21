qt_add_qml_module(CraftwardRealm
    STATIC
    URI Craftward.Realm
    DEPENDENCIES
        QtQuick
    SOURCES
        realmcontroller.cpp
        realmcontroller.h
)

target_include_directories(CraftwardRealm
    PRIVATE
        "${PROJECT_SOURCE_DIR}/src"
)

target_link_libraries(CraftwardRealm
    PRIVATE
        CraftwardCoreFfiSupport
        Qt6::Gui
        Qt6::Qml
        Ward::Core
        "${CRAFTWARD_VIRTUALIZATION_FRAMEWORK}"
)

if(BUILD_TESTING)
    qt_add_executable(CraftwardRealmControllerTest
        tests/realmcontrollertest.cpp
    )
    target_include_directories(CraftwardRealmControllerTest
        PRIVATE
            "${PROJECT_SOURCE_DIR}/src"
    )
    target_link_libraries(CraftwardRealmControllerTest
        PRIVATE
            CraftwardRealm
            Qt6::Gui
            Qt6::Test
            "${CRAFTWARD_VIRTUALIZATION_FRAMEWORK}"
    )
    add_test(
        NAME CraftwardRealmControllerTest
        COMMAND CraftwardRealmControllerTest
    )
endif()
