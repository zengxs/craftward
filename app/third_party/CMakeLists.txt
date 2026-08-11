set(scintilla_source_dir "${CMAKE_CURRENT_SOURCE_DIR}/scintilla")

add_library(CraftwardScintilla STATIC
    "${scintilla_source_dir}/cocoa/InfoBar.mm"
    "${scintilla_source_dir}/cocoa/PlatCocoa.mm"
    "${scintilla_source_dir}/cocoa/ScintillaCocoa.mm"
    "${scintilla_source_dir}/cocoa/ScintillaView.mm"
    "${scintilla_source_dir}/src/AutoComplete.cxx"
    "${scintilla_source_dir}/src/CallTip.cxx"
    "${scintilla_source_dir}/src/CaseConvert.cxx"
    "${scintilla_source_dir}/src/CaseFolder.cxx"
    "${scintilla_source_dir}/src/CellBuffer.cxx"
    "${scintilla_source_dir}/src/ChangeHistory.cxx"
    "${scintilla_source_dir}/src/CharClassify.cxx"
    "${scintilla_source_dir}/src/CharacterCategoryMap.cxx"
    "${scintilla_source_dir}/src/CharacterType.cxx"
    "${scintilla_source_dir}/src/ContractionState.cxx"
    "${scintilla_source_dir}/src/DBCS.cxx"
    "${scintilla_source_dir}/src/Decoration.cxx"
    "${scintilla_source_dir}/src/Document.cxx"
    "${scintilla_source_dir}/src/EditModel.cxx"
    "${scintilla_source_dir}/src/EditView.cxx"
    "${scintilla_source_dir}/src/Editor.cxx"
    "${scintilla_source_dir}/src/Geometry.cxx"
    "${scintilla_source_dir}/src/Indicator.cxx"
    "${scintilla_source_dir}/src/KeyMap.cxx"
    "${scintilla_source_dir}/src/LineMarker.cxx"
    "${scintilla_source_dir}/src/MarginView.cxx"
    "${scintilla_source_dir}/src/PerLine.cxx"
    "${scintilla_source_dir}/src/PositionCache.cxx"
    "${scintilla_source_dir}/src/RESearch.cxx"
    "${scintilla_source_dir}/src/RunStyles.cxx"
    "${scintilla_source_dir}/src/ScintillaBase.cxx"
    "${scintilla_source_dir}/src/Selection.cxx"
    "${scintilla_source_dir}/src/Style.cxx"
    "${scintilla_source_dir}/src/UndoHistory.cxx"
    "${scintilla_source_dir}/src/UniConversion.cxx"
    "${scintilla_source_dir}/src/UniqueString.cxx"
    "${scintilla_source_dir}/src/ViewStyle.cxx"
    "${scintilla_source_dir}/src/XPM.cxx"
)
add_library(Scintilla::Cocoa ALIAS CraftwardScintilla)

target_include_directories(CraftwardScintilla
    SYSTEM
    PUBLIC
        "${scintilla_source_dir}/cocoa"
        "${scintilla_source_dir}/include"
    PRIVATE
        "${scintilla_source_dir}/src"
)

target_compile_features(CraftwardScintilla PRIVATE cxx_std_17)
target_compile_options(CraftwardScintilla
    PRIVATE
        "$<$<COMPILE_LANGUAGE:OBJCXX>:-fobjc-arc>"
)

target_link_libraries(CraftwardScintilla
    PUBLIC
        "${CRAFTWARD_COCOA_FRAMEWORK}"
        "${CRAFTWARD_QUARTZ_CORE_FRAMEWORK}"
)
