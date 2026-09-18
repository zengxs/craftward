qt_add_qml_module(CraftwardComponents
    STATIC
    URI Craftward.Components
    SOURCES
        "${PROJECT_SOURCE_DIR}/src/components/applicationclipboard.cpp"
        "${PROJECT_SOURCE_DIR}/src/components/applicationclipboard.h"
        "${PROJECT_SOURCE_DIR}/src/components/applicationfiles.cpp"
        "${PROJECT_SOURCE_DIR}/src/components/applicationfiles.h"
        "${PROJECT_SOURCE_DIR}/src/components/projectfilesmodel.cpp"
        "${PROJECT_SOURCE_DIR}/src/components/projectfilesmodel.h"
        "${PROJECT_SOURCE_DIR}/src/components/popupanchortoggle.cpp"
        "${PROJECT_SOURCE_DIR}/src/components/popupanchortoggle.h"
        "${PROJECT_SOURCE_DIR}/src/components/popuppositioner.cpp"
        "${PROJECT_SOURCE_DIR}/src/components/popuppositioner.h"
        "${PROJECT_SOURCE_DIR}/src/components/macos/typography.mm"
        "${PROJECT_SOURCE_DIR}/src/components/macos/windowmovehelper.mm"
        "${PROJECT_SOURCE_DIR}/src/components/typography.h"
        "${PROJECT_SOURCE_DIR}/src/components/windowmovehelper.h"
    QML_FILES
        AnimatedChevron.qml
        CollapsibleSplitPane.qml
        ConfirmationDialog.qml
        ContentHeader.qml
        CopyFeedbackButton.qml
        FrameTiming.js
        FrameTimingOverlay.qml
        IconButton.qml
        MenuComboBox.qml
        MarkupPartsView.qml
        MarkupSegmentView.qml
        MarkupCodeComment.qml
        MarkupSelectableText.qml
        MarkupSelectionHost.qml
        MarkupTable.qml
        ModalDialog.qml
        OverlayScrollBar.qml
        PanelActionButton.qml
        PanelTab.qml
        PrimaryButton.qml
        WindowMoveHandler.qml
        WindowPopover.qml
)

target_include_directories(CraftwardComponents
    PRIVATE
        "${PROJECT_SOURCE_DIR}/src/components"
)

target_link_libraries(CraftwardComponents
    PRIVATE
        CraftwardDesign
        CraftwardHighlightingplugin
        CraftwardMarkupplugin
        "${CRAFTWARD_APPKIT_FRAMEWORK}"
        "${CRAFTWARD_CORE_TEXT_FRAMEWORK}"
        "${CRAFTWARD_FOUNDATION_FRAMEWORK}"
        Qt6::Gui
        Qt6::Qml
        Qt6::Quick
        Qt6::QuickControls2
)

target_compile_options(CraftwardComponents
    PRIVATE
        "$<$<COMPILE_LANGUAGE:OBJCXX>:-fobjc-arc>"
)

if(BUILD_TESTING)
    qt_add_executable(CraftwardWorkbenchFilesTest
        "${PROJECT_SOURCE_DIR}/src/components/applicationfiles.cpp"
        "${PROJECT_SOURCE_DIR}/src/components/applicationfiles.h"
        "${PROJECT_SOURCE_DIR}/src/components/projectfilesmodel.cpp"
        "${PROJECT_SOURCE_DIR}/src/components/projectfilesmodel.h"
        "${PROJECT_SOURCE_DIR}/src/components/tests/workbenchfilestest.cpp"
    )
    set_target_properties(CraftwardWorkbenchFilesTest PROPERTIES QT_QML_MODULE_NO_IMPORT_SCAN TRUE)
    target_include_directories(CraftwardWorkbenchFilesTest PRIVATE "${PROJECT_SOURCE_DIR}/src/components")
    target_link_libraries(CraftwardWorkbenchFilesTest PRIVATE Qt6::Gui Qt6::Qml Qt6::Test)
    add_test(NAME CraftwardWorkbenchFilesTest COMMAND CraftwardWorkbenchFilesTest)
endif()
