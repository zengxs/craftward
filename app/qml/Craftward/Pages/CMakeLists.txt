qt_add_qml_module(CraftwardPages
    STATIC
    URI Craftward.Pages
    QML_FILES
        AboutPage.qml
        CodexHistoryPage.qml
        SettingsGeneralPage.qml
)

target_link_libraries(CraftwardPages
    PRIVATE
        CraftwardComponentsplugin
        CraftwardCodexplugin
        Qt6::Quick
        Qt6::QuickControls2
)
