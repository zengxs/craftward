qt_add_qml_module(CraftwardRealm
    STATIC
    URI Craftward.Realm
    SOURCES
        coreffi.h
        realm/realmcontroller.cpp
        realm/realmcontroller.h
)

target_include_directories(CraftwardRealm
    PRIVATE
        "${CMAKE_CURRENT_SOURCE_DIR}/.."
        "${CMAKE_CURRENT_SOURCE_DIR}/realm"
)

target_link_libraries(CraftwardRealm
    PRIVATE
        Qt6::Gui
        Qt6::Qml
        Ward::Core
        "${CRAFTWARD_VIRTUALIZATION_FRAMEWORK}"
)
