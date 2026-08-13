qt_add_qml_module(CraftwardRealm
    STATIC
    URI Craftward.Realm
    DEPENDENCIES
        QtQuick
    SOURCES
        "${PROJECT_SOURCE_DIR}/src/ward/coreffi.h"
        realmcontroller.cpp
        realmcontroller.h
)

target_include_directories(CraftwardRealm
    PRIVATE
        "${PROJECT_SOURCE_DIR}/src"
)

target_link_libraries(CraftwardRealm
    PRIVATE
        Qt6::Gui
        Qt6::Qml
        Ward::Core
        "${CRAFTWARD_VIRTUALIZATION_FRAMEWORK}"
)
