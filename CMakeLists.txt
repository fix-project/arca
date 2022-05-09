set(MODULE_COMBINER_SRC
  combine-modules.cc
  combine-modules.h
  generate-prefix-names.cc
  generate-prefix-names.h
  resolve-imports.cc
  resolve-imports.h
)
add_library(module-combiner STATIC ${MODULE_COMBINER_SRC})

add_executable(wasmlink "wasmlink.cc")
target_link_libraries("wasmlink" module-combiner wabt )
