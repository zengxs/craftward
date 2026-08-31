qt_add_qml_module(CraftwardHighlighting
    STATIC
    URI Craftward.Highlighting
    SOURCES
        "${PROJECT_SOURCE_DIR}/src/highlighting/syntaxdocumenthighlighter.cpp"
        "${PROJECT_SOURCE_DIR}/src/highlighting/syntaxdocumenthighlighter.h"
        "${PROJECT_SOURCE_DIR}/src/highlighting/syntaxhighlightingengine.cpp"
        "${PROJECT_SOURCE_DIR}/src/highlighting/syntaxhighlightingengine.h"
)

qt_add_protobuf(CraftwardHighlighting
    GENERATE_PACKAGE_SUBFOLDERS
    PROTO_FILES
        "${PROJECT_SOURCE_DIR}/../proto/ward/highlighting/v1/highlight.proto"
    PROTO_INCLUDES
        "${PROJECT_SOURCE_DIR}/../proto"
)

target_include_directories(CraftwardHighlighting
    PUBLIC
        "$<BUILD_INTERFACE:${PROJECT_SOURCE_DIR}/src>"
    PRIVATE
        "$<BUILD_INTERFACE:${PROJECT_SOURCE_DIR}/src/highlighting>"
        "$<BUILD_INTERFACE:${CMAKE_CURRENT_BINARY_DIR}/ward/highlighting/v1>"
)

target_link_libraries(CraftwardHighlighting
    PRIVATE
        CraftwardCoreFfiSupport
        Qt6::Concurrent
        Qt6::Gui
        Qt6::Protobuf
        Qt6::Qml
        Qt6::Quick
        Ward::Core
)

if(BUILD_TESTING)
    qt_add_executable(CraftwardSyntaxHighlightingTest
        "${PROJECT_SOURCE_DIR}/src/highlighting/tests/syntaxhighlightingtest.cpp"
    )
    target_link_libraries(CraftwardSyntaxHighlightingTest
        PRIVATE
            CraftwardHighlighting
            Qt6::Gui
            Qt6::Quick
            Qt6::Test
            "${CRAFTWARD_VIRTUALIZATION_FRAMEWORK}"
    )
    add_test(
        NAME CraftwardSyntaxHighlightingTest
        COMMAND CraftwardSyntaxHighlightingTest
    )
endif()
