qt_add_qml_module(CraftwardEditor
    STATIC
    URI Craftward.Editor
    SOURCES
        ../../../src/editor/scintillaeditorbackend.h
        ../../../src/editor/scintillaeditorbackend.mm
    QML_FILES
        CodeEditor.qml
)

target_include_directories(CraftwardEditor
    PRIVATE
        ../../../src/editor
)

target_compile_options(CraftwardEditor
    PRIVATE
        "$<$<COMPILE_LANGUAGE:OBJCXX>:-fobjc-arc>"
)

target_link_libraries(CraftwardEditor
    PRIVATE
        Qt6::Gui
        Qt6::Qml
        Qt6::Quick
        Qt6::QuickControls2
        Scintilla::Cocoa
)
