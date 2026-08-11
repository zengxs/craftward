qt_add_qml_module(CraftwardComponents
    STATIC
    URI Craftward.Components
    SOURCES
        ../../../src/ui/applicationfonts.h
        ../../../src/ui/typography.cpp
        ../../../src/ui/typography.h
        "${CRAFTWARD_APPLICATION_FONTS_SOURCE}"
    QML_FILES
        ConfirmationDialog.qml
        IconButton.qml
        ModalDialog.qml
        OverlayScrollBar.qml
        PrimaryButton.qml
        WindowMoveHandler.qml
)

target_include_directories(CraftwardComponents
    PRIVATE
        ../../../src/ui
)

target_link_libraries(CraftwardComponents
    PRIVATE
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
