cmake_minimum_required(VERSION 3.22)
if(POLICY CMP0144)
    cmake_policy(SET CMP0144 NEW)
endif()
project(CraftwardInlinePrototype LANGUAGES CXX)

# Standalone, throwaway experiment. The production app does not link this target.
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
find_package(Qt6 6.11 REQUIRED COMPONENTS Quick QuickControls2)
qt_standard_project_setup(REQUIRES 6.11)

qt_add_executable(CraftwardInlinePrototype MACOSX_BUNDLE main.cpp MessageSelection.h MessageSelection.cpp)
qt_add_resources(CraftwardInlinePrototype prototype_qml
    PREFIX "/"
    FILES Main.qml InlineBlock.qml SelectionMain.qml SelectionSegment.qml
        SelectionText.qml SelectionProbe.qml SelectionFixtures.js TooltipProbe.qml InlineHelpTip.qml
)
target_link_libraries(CraftwardInlinePrototype PRIVATE Qt6::Quick Qt6::QuickControls2)
set_target_properties(CraftwardInlinePrototype PROPERTIES
    MACOSX_BUNDLE_GUI_IDENTIFIER "app.craftward.prototype.inline"
    MACOSX_BUNDLE_BUNDLE_NAME "Craftward Inline Prototype"
)
