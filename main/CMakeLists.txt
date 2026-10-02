set(UC_DIR "${CMAKE_CURRENT_LIST_DIR}/use_cases")

idf_component_register(
    SRCS
        "main.c"
        "registry.c"
        "${UC_DIR}/toggle-physical-led/logic.c"
        "${UC_DIR}/toggle-physical-led/commands.c"
        "${UC_DIR}/toggle-physical-led/setup.c"
        "${UC_DIR}/read-analog-sensor/logic.c"
        "${UC_DIR}/read-analog-sensor/commands.c"
        "${UC_DIR}/read-analog-sensor/setup.c"
        "${UC_DIR}/fetch-and-save/logic.c"
        "${UC_DIR}/fetch-and-save/commands.c"
        "${UC_DIR}/fetch-and-save/setup.c"
        "${UC_DIR}/read-and-insert/logic.c"
        "${UC_DIR}/read-and-insert/commands.c"
        "${UC_DIR}/read-and-insert/setup.c"
        "${UC_DIR}/fetch-and-uart/logic.c"
        "${UC_DIR}/fetch-and-uart/commands.c"
        "${UC_DIR}/fetch-and-uart/setup.c"
    INCLUDE_DIRS
        "."
        "${UC_DIR}/toggle-physical-led"
        "${UC_DIR}/read-analog-sensor"
        "${UC_DIR}/fetch-and-save"
        "${UC_DIR}/read-and-insert"
        "${UC_DIR}/fetch-and-uart"
)

# Force-keep each plugin's registration constructor. IDF links with
# --gc-sections; without these references the .ctors slot and the constructor
# body are discarded even though they come from a compiled translation unit.
set(UC_MODULES
    toggle_physical_led
    read_analog_sensor
    fetch_and_save
    read_and_insert
    fetch_and_uart
)
set(UC_KEEP_FLAGS "")
foreach(uc ${UC_MODULES})
    list(APPEND UC_KEEP_FLAGS "-Wl,--undefined=_uc_register_${uc}")
endforeach()
target_link_options(${COMPONENT_LIB} INTERFACE ${UC_KEEP_FLAGS})
