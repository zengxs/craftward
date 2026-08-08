qt_add_executable(Craftward
    ../../../src/main.cpp
)

set_source_files_properties(
    "${CRAFTWARD_ENTITLEMENTS_FILE}"
    PROPERTIES HEADER_FILE_ONLY TRUE
)
target_sources(Craftward PRIVATE "${CRAFTWARD_ENTITLEMENTS_FILE}")

qt_add_qml_module(Craftward
    URI Craftward.App
    QML_FILES
        Main.qml
        MainWindow.qml
        SettingsWindow.qml
)

target_link_libraries(Craftward
    PRIVATE
        CraftwardComponentsplugin
        CraftwardFeaturesLegalplugin
        CraftwardPagesplugin
        Qt6::Quick
        Qt6::QuickControls2
)

target_compile_definitions(Craftward
    PRIVATE
        CRAFTWARD_BUILD_NUMBER="${CRAFTWARD_BUILD_NUMBER}"
        CRAFTWARD_COMMIT_HASH="${CRAFTWARD_COMMIT_HASH}"
        CRAFTWARD_VERSION="${PROJECT_VERSION}"
)

set_target_properties(Craftward PROPERTIES
    MACOSX_BUNDLE TRUE
    MACOSX_BUNDLE_BUNDLE_NAME "Craftward"
    MACOSX_BUNDLE_BUNDLE_VERSION "${CRAFTWARD_BUILD_NUMBER}"
    MACOSX_BUNDLE_GUI_IDENTIFIER "${CRAFTWARD_BUNDLE_IDENTIFIER}"
    MACOSX_BUNDLE_SHORT_VERSION_STRING "${PROJECT_VERSION}"
    RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}"
)

install(TARGETS Craftward
    BUNDLE DESTINATION .
    RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}"
)

if(CRAFTWARD_SIGN_IDENTITY STREQUAL "-")
    set(craftward_deploy_sign_options "-codesign=-")
else()
    set(craftward_deploy_sign_options
        "-sign-for-notarization=${CRAFTWARD_SIGN_IDENTITY}"
    )
endif()

qt_generate_deploy_qml_app_script(
    TARGET Craftward
    OUTPUT_SCRIPT craftward_deploy_script
    DEPLOY_TOOL_OPTIONS ${craftward_deploy_sign_options}
)
install(SCRIPT "${craftward_deploy_script}")
