qt_add_qml_module(CraftwardFeaturesRealm
    STATIC
    URI Craftward.Features.Realm
    QML_FILES
        RealmDisplayWindow.qml
        RealmManagerWindow.qml
        RealmPage.qml
)

target_link_libraries(CraftwardFeaturesRealm
    PRIVATE
        CraftwardComponentsplugin
        CraftwardDesign
        CraftwardRealmplugin
        Qt6::Quick
        Qt6::QuickControls2
        Qt6::QuickDialogs2
)
