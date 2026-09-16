qt_add_qml_module(CraftwardEditor
    STATIC
    URI Craftward.Editor
    SOURCES
        "${PROJECT_SOURCE_DIR}/src/editor/scintillaeditorbackend.h"
        "${PROJECT_SOURCE_DIR}/src/editor/scintillaeditorbackend.cpp"
        "${PROJECT_SOURCE_DIR}/src/editor/scintillaimageitem_p.h"
        "${PROJECT_SOURCE_DIR}/src/editor/scintillaimageitem.cpp"
        "${PROJECT_SOURCE_DIR}/src/editor/scintillaquickadapter_p.h"
        "${PROJECT_SOURCE_DIR}/src/editor/scintillaquickadapter.cpp"
        "${PROJECT_SOURCE_DIR}/src/editor/scintillaquickplatform_p.h"
        "${PROJECT_SOURCE_DIR}/src/editor/scintillaquickplatform.cpp"
        "${PROJECT_SOURCE_DIR}/src/editor/scintillaquickwindow.cpp"
    QML_FILES
        CodeEditor.qml
)

target_include_directories(CraftwardEditor
    PRIVATE
        "${PROJECT_SOURCE_DIR}/src/editor"
        "${PROJECT_SOURCE_DIR}/third_party/scintilla/src"
)

target_link_libraries(CraftwardEditor
    PRIVATE
        CraftwardComponentsplugin
        Qt6::Gui
        Qt6::Qml
        Qt6::Quick
        Qt6::QuickControls2
        Scintilla::Core
)

if(BUILD_TESTING)
    qt_add_executable(CraftwardScintillaQuickTest
        "${PROJECT_SOURCE_DIR}/src/editor/tests/scintillaquicktest.cpp"
    )
    qt_add_resources(CraftwardScintillaQuickTest scintilla_editor_test_views
        PREFIX "/editor-tests"
        BASE "${PROJECT_SOURCE_DIR}/qml/Craftward"
        FILES
            "${PROJECT_SOURCE_DIR}/qml/Craftward/Pages/FileContentView.qml"
            "${PROJECT_SOURCE_DIR}/qml/Craftward/Features/Legal/LegalTextView.qml"
    )
    target_include_directories(CraftwardScintillaQuickTest PRIVATE "${PROJECT_SOURCE_DIR}/src/editor")
    target_link_libraries(CraftwardScintillaQuickTest PRIVATE
        CraftwardEditor CraftwardEditorplugin Scintilla::Core Qt6::Quick Qt6::Test Qt6::GuiPrivate
        "${CRAFTWARD_VIRTUALIZATION_FRAMEWORK}"
    )
    add_test(NAME CraftwardScintillaQuickTest COMMAND CraftwardScintillaQuickTest)
    set_tests_properties(CraftwardScintillaQuickTest PROPERTIES
        ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software"
        TIMEOUT 45
    )
    add_test(NAME CraftwardScintillaQuickTest.fractional COMMAND CraftwardScintillaQuickTest)
    set_tests_properties(CraftwardScintillaQuickTest.fractional PROPERTIES
        ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software;QT_SCALE_FACTOR=1.25"
        TIMEOUT 45
    )
endif()
