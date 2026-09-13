include_guard(GLOBAL)
include("${CMAKE_CURRENT_LIST_DIR}/contour-dependencies.cmake")
include("${CMAKE_CURRENT_LIST_DIR}/contour-fonts.cmake")

# Craftward owns the embedded target graph. No Contour CMakeLists.txt is evaluated.
function(craftward_contour_library target)
    set(sources ${ARGN})
    list(TRANSFORM sources PREPEND "${contour_source}/src/")
    if(target STREQUAL "contour_core")
        add_library(${target} OBJECT ${sources})
    else()
        add_library(${target} STATIC ${sources})
    endif()
    set_target_properties(${target} PROPERTIES
        CXX_STANDARD 23 OBJCXX_STANDARD 23
        CXX_STANDARD_REQUIRED ON OBJCXX_STANDARD_REQUIRED ON
        AUTOMOC OFF AUTORCC OFF
    )
    target_include_directories(${target} PUBLIC "${contour_source}/src")
    # Apple libc++ still gates the stop-token implementation used by Contour.
    target_compile_options(${target} PRIVATE -fexperimental-library "$<$<CONFIG:Debug>:-Og>")
endfunction()

function(craftward_add_contour)
    get_filename_component(source_root "${CMAKE_CURRENT_FUNCTION_LIST_DIR}/.." ABSOLUTE)
    set(contour_source "${source_root}/contour")
    set(contour_binary "${CMAKE_CURRENT_BINARY_DIR}/contour-embedded")
    set(CMAKE_AUTOMOC OFF)
    set(CMAKE_AUTOUIC OFF)
    set(CMAKE_FIND_PACKAGE_TARGETS_GLOBAL ON)
    find_package(Threads REQUIRED)
    find_package(Qt6 6.11 REQUIRED COMPONENTS
        Core Gui GuiPrivate Quick QuickControls2 QuickTemplates2 Qml
        Network Multimedia Widgets DBus ShaderTools)
    find_library(contour_carbon Carbon REQUIRED)
    craftward_add_contour_dependencies("${source_root}")
    craftward_add_contour_fonts("${source_root}")

    # Derive the embedded library's identity from the pinned source, not Craftward's version.
    set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${contour_source}/metainfo.xml")
    file(STRINGS "${contour_source}/metainfo.xml" release_line REGEX "<release version=\"[0-9]" LIMIT_COUNT 1)
    string(REGEX MATCH "([0-9]+)\\.([0-9]+)\\.([0-9]+)" version "${release_line}")
    if(NOT version)
        message(FATAL_ERROR "Cannot determine the embedded Contour version.")
    endif()
    set(version_major "${CMAKE_MATCH_1}")
    set(version_minor "${CMAKE_MATCH_2}")
    set(version_patch "${CMAKE_MATCH_3}")

    craftward_contour_library(crispy-core
        crispy/App.cpp
        crispy/BufferObject.cpp
        crispy/CLI.cpp
        crispy/StackTrace.cpp
        crispy/Environment.cpp
        crispy/InterpolatedString.cpp
        crispy/LogSink.cpp
        crispy/LogStore.cpp
        crispy/UserInfo.cpp
        crispy/Utils.cpp
    )
    craftward_contour_library(net
        net/AsyncBufferedReader.cpp
        net/DefaultEventSource.cpp
        net/Diagnostics.cpp
        net/EpollEventSource.cpp
        net/EventLoop.cpp
        net/HttpServer.cpp
        net/KqueueEventSource.cpp
        net/WriteQueue.cpp
        net/PollEventSource.cpp
        net/platform/SystemPipe.cpp
        net/platform/WinsockInit.cpp
        net/testing/InMemoryTransport.cpp
        net/posix/AcceptLoop.cpp
        net/posix/PosixListener.cpp
        net/posix/PosixSocket.cpp
        net/posix/SocketsPosix.cpp
        net/posix/UnixListener.cpp
        net/TlsDisabled.cpp
    )
    craftward_contour_library(text_shaper
        text_shaper/Font.cpp
        text_shaper/FontLocatorProvider.cpp
        text_shaper/MockFontLocator.cpp
        text_shaper/OpenShaper.cpp
        text_shaper/Shaper.cpp
        text_shaper/CoreTextLocator.mm
    )
    craftward_contour_library(vtpty
        vtpty/ChannelPty.cpp
        vtpty/MockPty.cpp
        vtpty/MockViewPty.cpp
        vtpty/Process_unix.cpp
        vtpty/Pty.cpp
        vtpty/SpawnLadder.cpp
        vtpty/UnixPty.cpp
        vtpty/UnixUtils.cpp
    )
    craftward_contour_library(vtparser
        vtparser/Parser.cpp
    )
    craftward_contour_library(vtbackend
        vtbackend/Capabilities.cpp
        vtbackend/Charset.cpp
        vtbackend/DesktopNotification.cpp
        vtbackend/ProgressState.cpp
        vtbackend/Color.cpp
        vtbackend/ColorPalette.cpp
        vtbackend/CommandBlocks.cpp
        vtbackend/PromptRegion.cpp
        vtbackend/Functions.cpp
        vtbackend/Grid.cpp
        vtbackend/FileUrl.cpp
        vtbackend/HintModeHandler.cpp
        vtbackend/Image.cpp
        vtbackend/InputBinding.cpp
        vtbackend/InputGenerator.cpp
        vtbackend/Line.cpp
        vtbackend/LineSoA.cpp
        vtbackend/MatchModes.cpp
        vtbackend/MessageParser.cpp
        vtbackend/MockTerm.cpp
        vtbackend/RenderBuffer.cpp
        vtbackend/RenderBufferBuilder.cpp
        vtbackend/KittyClipboard.cpp
        vtbackend/KittyGraphics.cpp
        vtbackend/TextSizing.cpp
        vtbackend/Screen.cpp
        vtbackend/SemanticBlockTracker.cpp
        vtbackend/Selector.cpp
        vtbackend/SoAClusterWriter.cpp
        vtbackend/Sequence.cpp
        vtbackend/SixelParser.cpp
        vtbackend/regis/ReGISColor.cpp
        vtbackend/regis/ReGISRasterizer.cpp
        vtbackend/regis/ReGISContext.cpp
        vtbackend/regis/ReGISFont.cpp
        vtbackend/regis/ReGISTextRasterizer.cpp
        vtbackend/regis/ReGISParser.cpp
        vtbackend/StatusLineBuilder.cpp
        vtbackend/Terminal.cpp
        vtbackend/VTType.cpp
        vtbackend/VTWriter.cpp
        vtbackend/Viewport.cpp
        vtbackend/ViInputHandler.cpp
        vtbackend/ViCommands.cpp
        vtbackend/JumpHistory.cpp
        vtbackend/Primitives.cpp
    )
    craftward_contour_library(vtworkspace
        vtworkspace/LayoutTree.cpp
        vtworkspace/Pane.cpp
        vtworkspace/Tab.cpp
        vtworkspace/SessionModel.cpp
    )
    craftward_contour_library(vthost
        vthost/Daemon.cpp
        vthost/ConnectionAcceptor.cpp
        vthost/NativeSession.cpp
        vthost/SessionHost.cpp
        vthost/ServiceControl.cpp
        vthost/ServiceControl_win32.cpp
        vthost/SessionSettings.cpp
        vthost/client/NativeClient.cpp
        vthost/client/LayoutReconstruction.cpp
        vthost/imsg/CommandArgv.cpp
        vthost/imsg/Identify.cpp
        vthost/imsg/ImsgCodec.cpp
        vthost/client/ScreenMirror.cpp
        vthost/GridWire.cpp
        vthost/proto/Pdu.cpp
        vthost/proto/PduTrace.cpp
        vthost/proto/Wire.cpp
        vthost/tmux/ControlModeParser.cpp
        vthost/tmux/ControlModeSpawn.cpp
        vthost/tmux/ControlOutput.cpp
        vthost/tmux/ControlSession.cpp
        vthost/tmux/ImsgServer.cpp
        vthost/tmux/LayoutString.cpp
        vthost/tmux/TmuxClientModel.cpp
        vthost/tmux/TmuxGateway.cpp
    )
    craftward_contour_library(vtrasterizer
        vtrasterizer/BackgroundRenderer.cpp
        vtrasterizer/BoxDrawingRenderer.cpp
        vtrasterizer/CursorRenderer.cpp
        vtrasterizer/DecorationRenderer.cpp
        vtrasterizer/ImageRenderer.cpp
        vtrasterizer/Pixmap.cpp
        vtrasterizer/RenderTarget.cpp
        vtrasterizer/ReGISFontRasterizer.cpp
        vtrasterizer/Renderer.cpp
        vtrasterizer/TextClusterGrouper.cpp
        vtrasterizer/TextRenderer.cpp
        vtrasterizer/Utils.cpp
    )
    craftward_contour_library(contour_core
        contour/session/HyperlinkTooltip.cpp
        contour/window/TabLabel.cpp
        contour/window/CommandPaletteModel.cpp
        contour/window/ContextMenuModel.cpp
        contour/ContourGuiApp.cpp
        contour/session/PaneProxy.cpp
        contour/session/SessionFactory.cpp
        contour/window/SettingsController.cpp
        contour/session/TerminalSession.cpp
        contour/display/TerminalAccessible.cpp
        contour/session/TerminalSessionManager.cpp
        contour/window/PopupShadowImageProvider.cpp
        contour/window/UiStyleProvider.cpp
        contour/window/WindowControlStyleProvider.cpp
        contour/window/WindowController.cpp
        contour/session/SessionInput.cpp
        contour/session/FontControl.cpp
        contour/session/SpawnCommand.cpp
        contour/remote/NativeController.cpp
        contour/remote/RemoteController.cpp
        contour/remote/RemoteLayout.cpp
        contour/remote/TmuxController.cpp
    )
    craftward_contour_library(contour_config
        contour/config/Actions.cpp
        contour/config/AtomicFileWrite.cpp
        contour/config/Config.cpp
        contour/config/GuiConfigStore.cpp
        contour/config/LayoutBuilder.cpp
        contour/config/LayoutStore.cpp
        contour/config/WindowControlStyle.cpp
    )
    craftward_contour_library(contour_command
        contour/command/Command.cpp
        contour/command/CommandCatalog.cpp
        contour/command/CommandHistory.cpp
        contour/command/CommandHistoryStore.cpp
        contour/command/ContextMenu.cpp
        contour/command/FuzzyFilter.cpp
        contour/command/Shortcut.cpp
        contour/command/TitleBarContextMenu.cpp
    )
    craftward_contour_library(contour_cli
        contour/cli/CaptureScreen.cpp
        contour/cli/ContourApp.cpp
        contour/cli/ShellIntegration.cpp
    )
    craftward_contour_library(contour_input
        contour/input/KeyMapping.cpp
        contour/input/KeyboardLayout.cpp
        contour/input/MouseMapping.cpp
    )
    craftward_contour_library(contour_platform
        contour/platform/Audio.cpp
        contour/platform/BlurBehind.cpp
        contour/platform/Clipboard.cpp
        contour/platform/DBusNotificationTransport.cpp
        contour/platform/FreeDesktopNotifier.cpp
        contour/platform/Notifier.cpp
        contour/platform/QtExternalLauncher.cpp
        contour/platform/SpeechSynthesizer.cpp
        contour/platform/NativeWindowFrame.cpp
        contour/platform/PopupShadow.cpp
        contour/platform/WaylandWindowShadow.cpp
        contour/platform/Win32WindowFrame.cpp
        contour/platform/WindowShadow.cpp
        contour/platform/WindowShadowTiles.cpp
        contour/platform/X11WindowShadow.cpp
        contour/platform/XcbProperty.cpp
        contour/platform/CocoaWindowFrame.mm
    )
    craftward_contour_library(ContourTerminalDisplay
        contour/display/ContentScale.cpp
        contour/display/RhiRenderer.cpp
        contour/display/ShaderConfig.cpp
        contour/display/TerminalDisplay.cpp
        contour/display/TerminalRenderNode.cpp
    )

    target_link_libraries(crispy-core PUBLIC
        unicode::unicode Microsoft.GSL::GSL boxed-cpp::boxed-cpp
        reflection-cpp::reflection-cpp Threads::Threads)
    # These facilities are part of the supported macOS SDK baseline.
    target_compile_definitions(crispy-core PUBLIC
        HAVE_BACKTRACE HAVE_BACKTRACE_SYMBOLS HAVE_CXXABI_H HAVE_DLADDR
        HAVE_DLFCN_H HAVE_DLSYM HAVE_EXECINFO_H HAVE_SYS_SELECT_H HAVE_UNWIND_H)
    target_link_libraries(net PUBLIC Threads::Threads)
    target_link_libraries(text_shaper PRIVATE
        unicode::unicode Microsoft.GSL::GSL boxed-cpp::boxed-cpp
        Freetype::Freetype HarfBuzz::HarfBuzz Cairo::Cairo
        "${CRAFTWARD_FOUNDATION_FRAMEWORK}" "${CRAFTWARD_APPKIT_FRAMEWORK}"
        "${CRAFTWARD_CORE_TEXT_FRAMEWORK}")
    target_link_libraries(vtpty PUBLIC crispy-core Microsoft.GSL::GSL PRIVATE util)
    target_link_libraries(vtparser PUBLIC Microsoft.GSL::GSL unicode::unicode)
    target_link_libraries(vtbackend PUBLIC
        Microsoft.GSL::GSL Threads::Threads crispy-core unicode::unicode vtparser vtpty)
    target_compile_definitions(vtbackend PRIVATE
        LIBTERMINAL_VERSION_MAJOR=${version_major}
        LIBTERMINAL_VERSION_MINOR=${version_minor}
        LIBTERMINAL_VERSION_PATCH=${version_patch}
        LIBTERMINAL_VERSION_STRING="${version}"
        LIBTERMINAL_NAME="contour")
    target_link_libraries(vtworkspace PUBLIC vtbackend crispy-core)
    target_link_libraries(vthost PUBLIC net vtworkspace vtbackend vtpty crispy-core)
    target_link_libraries(vtrasterizer PUBLIC vtbackend crispy-core text_shaper)
    target_link_libraries(contour_config PUBLIC
        crispy-core vtbackend vtpty vtrasterizer vtworkspace yaml-cpp::yaml-cpp)
    target_compile_definitions(contour_config PUBLIC
        CONTOUR_VERSION_MAJOR=${version_major}
        CONTOUR_VERSION_MINOR=${version_minor}
        CONTOUR_VERSION_PATCH=${version_patch}
        CONTOUR_VERSION_STRING="${version}"
        CONTOUR_PROJECT_SOURCE_DIR="${contour_source}"
        CONTOUR_APP_ID="org.contourterminal.Contour"
        CONTOUR_FRONTEND_GUI)
    target_link_libraries(contour_command PUBLIC contour_config)
    target_link_libraries(contour_cli PUBLIC contour_config PRIVATE vthost)
    target_link_libraries(contour_input PUBLIC crispy-core vtbackend Qt6::Core Qt6::Gui "${contour_carbon}")
    target_link_libraries(contour_platform PUBLIC
        contour_config Qt6::Core Qt6::DBus Qt6::Gui Qt6::Multimedia "${CRAFTWARD_APPKIT_FRAMEWORK}")
    target_link_libraries(ContourTerminalDisplay PUBLIC
        vtrasterizer contour_config contour_input contour_platform
        Qt6::Core Qt6::Gui Qt6::GuiPrivate Qt6::Multimedia Qt6::Quick Qt6::QuickControls2)
    target_compile_definitions(ContourTerminalDisplay PRIVATE CONTOUR_BUILD_TYPE="${CMAKE_BUILD_TYPE}")
    target_link_libraries(contour_core PUBLIC
        contour_command contour_cli vtworkspace contour_input contour_platform ContourTerminalDisplay
        Qt6::Core Qt6::DBus Qt6::Multimedia Qt6::Network Qt6::Qml Qt6::QuickControls2
        Qt6::Widgets Qt6::QuickTemplates2 PRIVATE vthost)
    set_target_properties(contour_core contour_platform ContourTerminalDisplay PROPERTIES AUTOMOC ON)

    # Keep upstream resource paths and shader names used by the rendering implementation.
    target_sources(contour_core PRIVATE "${contour_source}/src/contour/resources.qrc")
    target_sources(ContourTerminalDisplay PRIVATE
        "${contour_source}/src/contour/display/DisplayResources.qrc")
    set_target_properties(contour_core ContourTerminalDisplay PROPERTIES AUTORCC ON)
    qt6_add_shaders(ContourTerminalDisplay "contour_rhi_shaders"
        PRECOMPILE OPTIMIZED PREFIX "/contour/display/shaders"
        BASE "${contour_source}/src/contour/display/shaders"
        FILES
            "${contour_source}/src/contour/display/shaders/background.vert"
            "${contour_source}/src/contour/display/shaders/background.frag"
            "${contour_source}/src/contour/display/shaders/text.vert"
            "${contour_source}/src/contour/display/shaders/text.frag")

    # ContourGuiApp inherits its CLI model, which still references the embedded shell data.
    # Reuse the source generator without evaluating upstream target or packaging rules.
    set(shells bash fish tcsh zsh)
    set(shell_scripts ${shells})
    set(cli_source "${contour_source}/src/contour/cli")
    list(TRANSFORM shell_scripts PREPEND "${cli_source}/shell-integration/shell-integration.")
    set(shell_data "${contour_binary}/ShellIntegrationData.hpp")
    add_custom_command(OUTPUT "${shell_data}"
        COMMAND "${CMAKE_COMMAND}" -E make_directory "${contour_binary}"
        COMMAND "${CMAKE_COMMAND}" "-DSHELLS=${shells}"
            "-DSOURCE_DIR=${cli_source}" "-DOUTPUT=${shell_data}"
            -P "${cli_source}/EmbedShellIntegration.cmake"
        DEPENDS "${cli_source}/EmbedShellIntegration.cmake" ${shell_scripts}
        COMMENT "Embedding Contour shell data" VERBATIM)
    target_sources(contour_cli PRIVATE "${shell_data}")
    target_include_directories(contour_cli PRIVATE "${contour_binary}")
endfunction()
