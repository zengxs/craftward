qt_add_qml_module(CraftwardComponents
    STATIC
    URI Craftward.Components
    SOURCES
        "${PROJECT_SOURCE_DIR}/src/components/macos/typography.mm"
        "${PROJECT_SOURCE_DIR}/src/components/macos/windowmovehelper.mm"
        "${PROJECT_SOURCE_DIR}/src/components/typography.h"
        "${PROJECT_SOURCE_DIR}/src/components/windowmovehelper.h"
    QML_FILES
        ConfirmationDialog.qml
        IconButton.qml
        MenuComboBox.qml
        MarkupDocumentView.qml
        ModalDialog.qml
        OverlayScrollBar.qml
        PrimaryButton.qml
        WindowMoveHandler.qml
)

target_include_directories(CraftwardComponents
    PRIVATE
        "${PROJECT_SOURCE_DIR}/src/components"
)

target_link_libraries(CraftwardComponents
    PRIVATE
        CraftwardDesign
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
