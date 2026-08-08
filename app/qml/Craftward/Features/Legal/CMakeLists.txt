qt_add_qml_module(CraftwardFeaturesLegal
    STATIC
    URI Craftward.Features.Legal
    SOURCES
        ../../../../src/resourcetextreader.cpp
        ../../../../src/resourcetextreader.h
    QML_FILES
        LegalTextView.qml
        LicenseTextDialog.qml
        ThirdPartyLicensesDialog.qml
)

target_include_directories(CraftwardFeaturesLegal
    PRIVATE
        ../../../../src
)

target_link_libraries(CraftwardFeaturesLegal
    PRIVATE
        CraftwardComponentsplugin
        Qt6::Quick
        Qt6::QuickControls2
)
