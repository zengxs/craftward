qt_add_qml_module(CraftwardPages
    STATIC
    URI Craftward.Pages
    QML_FILES
        ScaffoldPage.qml
)

target_link_libraries(CraftwardPages
    PRIVATE
        CraftwardComponentsplugin
        Qt6::Quick
        Qt6::QuickControls2
)
