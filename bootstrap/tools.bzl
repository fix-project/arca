"""LLVM utilities and the programs carried by the bootstrap seed."""

LLVM_TOOLS = {
    "llvm-ar": ["llvm-dlltool", "llvm-lib", "llvm-ranlib"],
    "llvm-as": [],
    "llvm-cvtres": [],
    "llvm-cxxfilt": [],
    "llvm-dis": [],
    "llvm-dwarfdump": [],
    "llvm-dwarfutil": [],
    "llvm-dwp": [],
    "llvm-link": [],
    "llvm-mc": [],
    "llvm-ml": ["llvm-ml64"],
    "llvm-nm": [],
    "llvm-objcopy": ["llvm-strip", "llvm-bitcode-strip", "llvm-install-name-tool", "llvm-extract-bundle-entry"],
    "llvm-objdump": ["llvm-otool"],
    "llvm-readobj": ["llvm-readelf"],
    "llvm-size": [],
    "llvm-strings": [],
    "llvm-symbolizer": ["llvm-addr2line"],
    "llvm-undname": [],
}

LLVM_TOOL_NAMES = [name for tool, aliases in LLVM_TOOLS.items() for name in [tool] + aliases]
COMMANDS = {
    "clang": "commands/clang.wasm",
    "lld": "usr/bin/ld.lld",
    "wasm2c": "commands/wasm2c.wasm",
    "wasm-merge": "commands/wasm-merge.wasm",
} | {name: "usr/bin/" + name for name in LLVM_TOOLS}
