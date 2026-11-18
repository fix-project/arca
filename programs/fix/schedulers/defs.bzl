"""Build a scheduler policy as a standalone Fix program."""
load("//programs/fix:defs.bzl", "fix_rust_program")

def _entry_impl(ctx):
    sources = []
    modules = []
    for source in ctx.files.srcs:
        copied = ctx.actions.declare_file(ctx.label.name + "/" + source.basename)
        ctx.actions.symlink(output = copied, target_file = source)
        sources.append(copied)
        modules.append("pub mod " + source.basename[:-3] + ";")
    entry = ctx.actions.declare_file(ctx.label.name + "/entry.rs")
    ctx.actions.write(entry, """#![no_std]
extern crate alloc;
%s
use fixwasm::{Error, Handle, Object, Tree};
#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;
#[fixwasm::apply]
fn apply(combination: &Handle<Object<Tree>>) -> Result<fixscheduler::shell::Value, Error> {
    %s::run(combination)
}
""" % ("\n".join(modules), ctx.attr.module))
    return [DefaultInfo(files = depset([entry])), OutputGroupInfo(sources = depset(sources))]

_entry = rule(
    implementation = _entry_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = [".rs"]),
        "module": attr.string(mandatory = True),
    },
)

def fix_scheduler(name, modules = [], memories = 4, deps = []):
    _entry(name = name + "_entry", srcs = modules + [name + ".rs"], module = name)
    native.filegroup(name = name + "_sources", srcs = [":" + name + "_entry"], output_group = "sources")
    fix_rust_program(
        name = name,
        src = ":" + name + "_entry",
        srcs = modules + [name + ".rs", ":" + name + "_sources"],
        importers = True,
        rustc_flags = ["-Clink-arg=-zstack-size=8388608"],
        memories = memories,
        artifact_visibility = ["//visibility:public"],
        deps = ["//fix/scheduler:sdk"] + deps,
    )
