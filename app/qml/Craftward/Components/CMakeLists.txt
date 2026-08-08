qt_add_qml_module(CraftwardComponents
    STATIC
    URI Craftward.Components
    QML_FILES
        PrimaryButton.qml
        WindowMoveHandler.qml
)

target_link_libraries(CraftwardComponents
    PRIVATE
        Qt6::Quick
        Qt6::QuickControls2
)
