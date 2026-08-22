file(GLOB_RECURSE craftward_icon_files
    CONFIGURE_DEPENDS
    LIST_DIRECTORIES false
    "${CMAKE_CURRENT_SOURCE_DIR}/icons/*.svg"
)

qt_add_resources(Craftward "craftward_icons"
    PREFIX "/icons"
    BASE "${CMAKE_CURRENT_SOURCE_DIR}/icons"
    FILES
        ${craftward_icon_files}
)
