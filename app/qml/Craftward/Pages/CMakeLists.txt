qt_add_qml_module(CraftwardPages
    STATIC
    URI Craftward.Pages
    QML_FILES
        AboutPage.qml
        CodexApprovalActionState.qml
        CodexComposer.qml
        CodexComposerState.qml
        CodexHistoryPage.qml
        CodexInteractionCard.qml
        CodexTimelineView.qml
        CodexTurnControlState.qml
        SettingsGeneralPage.qml
)

target_link_libraries(CraftwardPages
    PRIVATE
        CraftwardComponentsplugin
        CraftwardCodexplugin
        Qt6::Quick
        Qt6::QuickControls2
)
