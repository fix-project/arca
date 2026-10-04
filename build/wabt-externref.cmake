function(replace_required path before after)
    file(READ "${path}" contents)
    string(FIND "${contents}" "${before}" position)
    if(position EQUAL -1)
        message(FATAL_ERROR "WABT externref patch does not match ${path}")
    endif()
    string(REPLACE "${before}" "${after}" contents "${contents}")
    file(WRITE "${path}" "${contents}")
endfunction()

replace_required("${SOURCE}/src/c-writer.cc"
[=[  Write(s_source_declarations, Newline());]=]
[=[  Write("#ifndef WASM_RT_EXTERNREF_IS_NULL", Newline(),
        "#define WASM_RT_EXTERNREF_IS_NULL(value) ((value) == wasm_rt_externref_null_value)",
        Newline(), "#endif", Newline());
  Write(s_source_declarations, Newline());]=])

replace_required("${SOURCE}/src/c-writer.cc"
[=[            Write(StackVar(0, Type::I32), " = (", StackVar(0),
                  " == ", GetReferenceNullValue(Type::ExternRef), ");",
                  Newline());]=]
[=[            Write(StackVar(0, Type::I32), " = WASM_RT_EXTERNREF_IS_NULL(",
                  StackVar(0), ");", Newline());]=])
