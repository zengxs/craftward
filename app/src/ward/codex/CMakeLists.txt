qt_add_qml_module(CraftwardCodex
    STATIC
    URI Craftward.Codex
    SOURCES
        codexhistorycontroller.cpp
        codexhistorycontroller_events.cpp
        codexhistorycontroller.h
        codexinteractionmodel.cpp
        codexinteractionmodel.h
        codexmodelcatalogmodel.cpp
        codexmodelcatalogmodel.h
        codexthreadmodel.cpp
        codexthreadmodel.h
        codextimelinemodel.cpp
        codextimelinemodel.h
)

qt_add_protobuf(CraftwardCodex
    GENERATE_PACKAGE_SUBFOLDERS
    PROTO_FILES
        "${PROJECT_SOURCE_DIR}/../proto/ward/codex/v1/history.proto"
    PROTO_INCLUDES
        "${PROJECT_SOURCE_DIR}/../proto"
)

target_include_directories(CraftwardCodex
    PUBLIC
        "$<BUILD_INTERFACE:${CMAKE_CURRENT_BINARY_DIR}/ward/codex/v1>"
    PRIVATE
        "${PROJECT_SOURCE_DIR}/src"
)

target_link_libraries(CraftwardCodex
    PRIVATE
        Qt6::Protobuf
        Qt6::Qml
        Ward::Core
)

if(BUILD_TESTING)
    qt_add_executable(CraftwardCodexHistoryControllerTest
        tests/codexhistorycontrollertest.cpp
    )
    target_include_directories(CraftwardCodexHistoryControllerTest
        PRIVATE
            "${PROJECT_SOURCE_DIR}/src"
    )
    target_link_libraries(CraftwardCodexHistoryControllerTest
        PRIVATE
            CraftwardCodex
            Qt6::Test
            "${CRAFTWARD_VIRTUALIZATION_FRAMEWORK}"
    )
    add_test(
        NAME CraftwardCodexHistoryControllerTest
        COMMAND CraftwardCodexHistoryControllerTest
    )
endif()
