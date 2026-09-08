add_library(CraftwardMarkup STATIC
    markupdocumentmodel.cpp
    markupdocumentmodel.h
    markuprenderplan.cpp
    markuprenderplan.h
)

qt_add_qml_module(CraftwardMarkup
    URI Craftward.Markup
    QML_FILES
        MarkupSelectionDecoration.qml
    SOURCES
        markuptextdocument.cpp
        markuptextdocument.h
        markupselection.cpp
        markupselection.h
        markupselectionbackground.cpp
        markupselectionbackground.h
)

qt_add_protobuf(CraftwardMarkup
    GENERATE_PACKAGE_SUBFOLDERS
    PROTO_FILES
        "${PROJECT_SOURCE_DIR}/../proto/ward/markup/v1/document.proto"
    PROTO_INCLUDES
        "${PROJECT_SOURCE_DIR}/../proto"
)

target_include_directories(CraftwardMarkup
    PUBLIC
        "$<BUILD_INTERFACE:${PROJECT_SOURCE_DIR}/src>"
        "$<BUILD_INTERFACE:${CMAKE_CURRENT_BINARY_DIR}/ward/markup/v1>"
)

target_link_libraries(CraftwardMarkup
    PRIVATE
        CraftwardCoreFfiSupport
        Qt6::Concurrent
        Qt6::Protobuf
        Qt6::Gui
        Qt6::Qml
        Qt6::Quick
        Ward::Core
)

if(BUILD_TESTING)
    qt_add_executable(CraftwardMarkupSemanticTest tests/markupsemantictest.cpp)
    target_link_libraries(CraftwardMarkupSemanticTest PRIVATE
        CraftwardMarkup CraftwardMarkupplugin Qt6::Quick Qt6::Protobuf Qt6::Test
        Ward::Core
        CraftwardComponentsplugin
        "${CRAFTWARD_VIRTUALIZATION_FRAMEWORK}")
    add_test(NAME CraftwardMarkupSemanticTest COMMAND CraftwardMarkupSemanticTest)

endif()
