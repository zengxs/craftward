qt_add_qml_module(CraftwardRealm
    STATIC
    URI Craftward.Realm
    DEPENDENCIES
        QtQuick
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

qt_add_qml_module(CraftwardCodex
    STATIC
    URI Craftward.Codex
    SOURCES
        codex/codexhistorycontroller.cpp
        codex/codexhistorycontroller.h
        codex/codexmessagemodel.cpp
        codex/codexmessagemodel.h
        codex/codexthreadmodel.cpp
        codex/codexthreadmodel.h
)

qt_add_protobuf(CraftwardCodex
    GENERATE_PACKAGE_SUBFOLDERS
    PROTO_FILES
        "${CMAKE_SOURCE_DIR}/../proto/ward/codex/v1/history.proto"
    PROTO_INCLUDES
        "${CMAKE_SOURCE_DIR}/../proto"
)

target_include_directories(CraftwardCodex
    PUBLIC
        "$<BUILD_INTERFACE:${CMAKE_CURRENT_BINARY_DIR}/ward/codex/v1>"
    PRIVATE
        "${CMAKE_CURRENT_SOURCE_DIR}/.."
        "${CMAKE_CURRENT_SOURCE_DIR}/codex"
)

target_link_libraries(CraftwardCodex
    PRIVATE
        Qt6::Concurrent
        Qt6::Protobuf
        Qt6::Qml
        Ward::Core
)
