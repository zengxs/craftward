qt_add_qml_module(CraftwardPages
    STATIC
    URI Craftward.Pages
    QML_FILES
        AboutPage.qml
        RealmPage.qml
        SettingsGeneralPage.qml
)

target_link_libraries(CraftwardPages
    PRIVATE
        CraftwardComponentsplugin
        CraftwardRealmplugin
        Qt6::Quick
        Qt6::QuickControls2
        Qt6::QuickDialogs2
)
