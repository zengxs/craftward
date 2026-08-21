qt_add_qml_module(CraftwardFeaturesLegal
    STATIC
    URI Craftward.Features.Legal
    SOURCES
        "${PROJECT_SOURCE_DIR}/src/features/legal/resourcetextreader.cpp"
        "${PROJECT_SOURCE_DIR}/src/features/legal/resourcetextreader.h"
    QML_FILES
        LegalTextView.qml
        LicenseTextDialog.qml
        ThirdPartyLicensesDialog.qml
)

target_include_directories(CraftwardFeaturesLegal
    PRIVATE
        "${PROJECT_SOURCE_DIR}/src/features/legal"
)

target_link_libraries(CraftwardFeaturesLegal
    PRIVATE
        CraftwardComponentsplugin
        CraftwardDesign
        CraftwardEditorplugin
        Qt6::Quick
        Qt6::QuickControls2
)
