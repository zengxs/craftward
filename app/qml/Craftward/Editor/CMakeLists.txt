qt_add_qml_module(CraftwardEditor
    STATIC
    URI Craftward.Editor
    SOURCES
        "${PROJECT_SOURCE_DIR}/src/editor/scintillaeditorbackend.h"
        "${PROJECT_SOURCE_DIR}/src/editor/scintillaeditorbackend.mm"
    QML_FILES
        CodeEditor.qml
)

target_include_directories(CraftwardEditor
    PRIVATE
        "${PROJECT_SOURCE_DIR}/src/editor"
)

target_compile_options(CraftwardEditor
    PRIVATE
        "$<$<COMPILE_LANGUAGE:OBJCXX>:-fobjc-arc>"
)

target_link_libraries(CraftwardEditor
    PRIVATE
        CraftwardComponentsplugin
        Qt6::Gui
        Qt6::Qml
        Qt6::Quick
        Qt6::QuickControls2
        Scintilla::Cocoa
)
