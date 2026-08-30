qt_add_executable(Craftward
    "${PROJECT_SOURCE_DIR}/src/app/applicationiconprovider.h"
    "${PROJECT_SOURCE_DIR}/src/app/applicationmenus.h"
    "${PROJECT_SOURCE_DIR}/src/app/macos/applicationcontroller.mm"
    "${PROJECT_SOURCE_DIR}/src/app/macos/applicationiconprovider.mm"
    "${PROJECT_SOURCE_DIR}/src/app/macos/applicationmenus.mm"
    "${PROJECT_SOURCE_DIR}/src/app/main.cpp"
)

qt_add_resources(craftward_legal_resources "${CRAFTWARD_LEGAL_QRC}")
target_sources(Craftward PRIVATE ${craftward_legal_resources})

set_source_files_properties(
    "${CRAFTWARD_ENTITLEMENTS_FILE}"
    PROPERTIES HEADER_FILE_ONLY TRUE
)
set_source_files_properties(
    "${CRAFTWARD_APP_ICON_FILE}"
    PROPERTIES MACOSX_PACKAGE_LOCATION "Resources"
)
set_source_files_properties(
    ${CRAFTWARD_FIRA_CODE_FONT_FILES}
    PROPERTIES MACOSX_PACKAGE_LOCATION "Resources/fonts/firacode"
)
target_sources(Craftward PRIVATE
    "${CRAFTWARD_ENTITLEMENTS_FILE}"
    "${CRAFTWARD_APP_ICON_FILE}"
    ${CRAFTWARD_FIRA_CODE_FONT_FILES}
)

qt_add_qml_module(Craftward
    URI Craftward.App
    SOURCES
        "${PROJECT_SOURCE_DIR}/src/app/applicationcontroller.cpp"
        "${PROJECT_SOURCE_DIR}/src/app/applicationcontroller.h"
    QML_FILES
        Main.qml
        MainWindow.qml
        SettingsWindow.qml
)

target_include_directories(Craftward
    PRIVATE
        "${PROJECT_SOURCE_DIR}/src"
        "${PROJECT_SOURCE_DIR}/src/app"
)

target_link_libraries(Craftward
    PRIVATE
        CraftwardCoreFfiSupport
        CraftwardComponentsplugin
        CraftwardCodex
        CraftwardCodexplugin
        CraftwardDesign
        CraftwardEditorplugin
        CraftwardFeaturesLegalplugin
        CraftwardFeaturesRealmplugin
        CraftwardLocalizationplugin
        CraftwardPagesplugin
        CraftwardRealmplugin
        Qt6::Quick
        Qt6::QuickControls2
        Qt6::QuickDialogs2
        Qt6::Svg
        Ward::Core
        "${CRAFTWARD_APPKIT_FRAMEWORK}"
)

target_compile_options(Craftward
    PRIVATE
        "$<$<COMPILE_LANGUAGE:OBJCXX>:-fobjc-arc>"
)

target_compile_definitions(Craftward
    PRIVATE
        CRAFTWARD_BUILD_NUMBER="${CRAFTWARD_BUILD_NUMBER}"
        CRAFTWARD_COMMIT_HASH="${CRAFTWARD_COMMIT_HASH}"
        CRAFTWARD_VERSION="${PROJECT_VERSION}"
)

if(BUILD_TESTING)
    qt_add_executable(CraftwardApplicationControllerTest
        "${PROJECT_SOURCE_DIR}/src/app/applicationcontroller.cpp"
        "${PROJECT_SOURCE_DIR}/src/app/applicationcontroller.h"
        "${PROJECT_SOURCE_DIR}/src/app/macos/applicationcontroller.mm"
        "${PROJECT_SOURCE_DIR}/src/app/tests/applicationcontrollertest.mm"
    )
    target_include_directories(CraftwardApplicationControllerTest
        PRIVATE
            "${PROJECT_SOURCE_DIR}/src"
            "${PROJECT_SOURCE_DIR}/src/app"
    )
    target_link_libraries(CraftwardApplicationControllerTest
        PRIVATE
            CraftwardRealm
            Qt6::Gui
            Qt6::Qml
            Qt6::Test
            "${CRAFTWARD_APPKIT_FRAMEWORK}"
    )
    target_compile_options(CraftwardApplicationControllerTest
        PRIVATE
            "$<$<COMPILE_LANGUAGE:OBJCXX>:-fobjc-arc>"
    )
    add_test(
        NAME CraftwardApplicationControllerTest
        COMMAND CraftwardApplicationControllerTest
    )
endif()

set_target_properties(Craftward PROPERTIES
    MACOSX_BUNDLE TRUE
    MACOSX_BUNDLE_BUNDLE_NAME "Craftward"
    MACOSX_BUNDLE_BUNDLE_VERSION "${CRAFTWARD_BUILD_NUMBER}"
    MACOSX_BUNDLE_GUI_IDENTIFIER "${CRAFTWARD_BUNDLE_IDENTIFIER}"
    MACOSX_BUNDLE_ICON_FILE "Ward.icns"
    MACOSX_BUNDLE_SHORT_VERSION_STRING "${PROJECT_VERSION}"
    RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}"
)

set(craftward_local_sign_options
    --force
    --sign "${CRAFTWARD_SIGN_IDENTITY}"
    --entitlements "${CRAFTWARD_ENTITLEMENTS_FILE}"
    --timestamp=none
)
if(NOT CRAFTWARD_SIGN_IDENTITY STREQUAL "-")
    list(APPEND craftward_local_sign_options --options runtime)
endif()

add_custom_target(CraftwardLocalSign ALL
    COMMAND
        "${CRAFTWARD_CODESIGN_EXECUTABLE}"
        ${craftward_local_sign_options}
        "$<TARGET_BUNDLE_DIR:Craftward>"
    DEPENDS
        Craftward
        "${CRAFTWARD_ENTITLEMENTS_FILE}"
    COMMENT "Signing Craftward for local execution"
    VERBATIM
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
