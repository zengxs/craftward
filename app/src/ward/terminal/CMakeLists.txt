# Only the implementation and its generated QML registration see Contour's C++23 headers.
set(CMAKE_CXX_STANDARD 23)
set(CMAKE_OBJCXX_STANDARD 23)

qt_add_qml_module(CraftwardTerminal
    STATIC
    URI Craftward.Terminal
    SOURCES
        loginshellcommand.cpp loginshellcommand_p.h
        terminalcontroller.cpp terminalcontroller.h
        terminalview.cpp terminalview.h
)
target_include_directories(CraftwardTerminal PUBLIC "${PROJECT_SOURCE_DIR}/src")
target_link_libraries(CraftwardTerminal PUBLIC Qt6::Quick PRIVATE contour_core)
target_compile_definitions(CraftwardTerminal PRIVATE
    CRAFTWARD_TERMINAL_CONFIG="${CMAKE_CURRENT_SOURCE_DIR}/contour.yml"
    CRAFTWARD_TERMINAL_FONT_DIRECTORY="${PROJECT_SOURCE_DIR}/assets/fonts/firacode"
)

if(BUILD_TESTING)
    add_executable(CraftwardTerminalInputTest tests/terminalinputtest.cpp)
    target_link_libraries(CraftwardTerminalInputTest PRIVATE contour_core)
    foreach(mode IN ITEMS default paste retry fatal large parser raw query batched)
        add_test(NAME "CraftwardTerminalInputTest.${mode}" COMMAND CraftwardTerminalInputTest "${mode}")
        set_tests_properties("CraftwardTerminalInputTest.${mode}" PROPERTIES TIMEOUT 3)
    endforeach()

    add_executable(CraftwardTerminalCursorTest tests/terminalcursortest.cpp)
    target_link_libraries(CraftwardTerminalCursorTest PRIVATE contour_core)
    add_test(NAME CraftwardTerminalCursorTest
        COMMAND CraftwardTerminalCursorTest "${CMAKE_CURRENT_SOURCE_DIR}/contour.yml")
    set_tests_properties(CraftwardTerminalCursorTest PROPERTIES TIMEOUT 5)

    qt_add_executable(CraftwardTerminalFontTest tests/terminalfonttest.cpp)
    target_link_libraries(CraftwardTerminalFontTest PRIVATE CraftwardTerminal contour_core Freetype::Freetype)
    add_test(NAME CraftwardTerminalFontTest.faces COMMAND CraftwardTerminalFontTest)
    add_test(NAME CraftwardTerminalFontTest.fallback
        COMMAND CraftwardTerminalFontTest --fallback "${CMAKE_CURRENT_SOURCE_DIR}/contour.yml")
    set_tests_properties(CraftwardTerminalFontTest.faces CraftwardTerminalFontTest.fallback PROPERTIES
        ENVIRONMENT "QT_QPA_PLATFORM=offscreen"
        TIMEOUT 20
    )

    add_library(CraftwardTerminalPublicHeaders OBJECT tests/publicheadercheck.cpp)
    set_target_properties(CraftwardTerminalPublicHeaders PROPERTIES CXX_STANDARD 20)
    target_link_libraries(CraftwardTerminalPublicHeaders PRIVATE CraftwardTerminal)

    qt_add_executable(CraftwardTerminalIntegrationTest tests/terminalintegrationtest.cpp)
    target_link_libraries(CraftwardTerminalIntegrationTest PRIVATE
        CraftwardTerminal CraftwardTerminalplugin CraftwardDesign contour_core
        Qt6::QuickControls2 Qt6::Test
    )
    qt_add_resources(CraftwardTerminalIntegrationTest "terminal_test_icons"
        PREFIX "/icons"
        BASE "${PROJECT_SOURCE_DIR}/assets/icons"
        FILES
            "${PROJECT_SOURCE_DIR}/assets/icons/hugeicons/square-terminal.svg"
            "${PROJECT_SOURCE_DIR}/assets/icons/hugeicons/cancel-01.svg"
            "${PROJECT_SOURCE_DIR}/assets/icons/hugeicons/add-01.svg"
    )
    target_compile_definitions(CraftwardTerminalIntegrationTest PRIVATE
        CRAFTWARD_TERMINAL_PANEL="${PROJECT_SOURCE_DIR}/qml/Craftward/Pages/TerminalPanel.qml"
    )
    add_dependencies(CraftwardTerminalIntegrationTest CraftwardTerminalPublicHeaders)
    add_test(NAME CraftwardTerminalIntegrationTest COMMAND CraftwardTerminalIntegrationTest)
    set_tests_properties(CraftwardTerminalIntegrationTest PROPERTIES TIMEOUT 45)
endif()
