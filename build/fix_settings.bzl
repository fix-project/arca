"""Fix evaluation budgets."""

load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")
load("@local_tools//:cores.bzl", "HOST_CPUS")

def _jobs(ctx):
    value = ctx.build_setting_value
    if value < 0:
        fail("fix-jobs must be nonnegative (0 uses available cores)")
    jobs = value or HOST_CPUS
    return [BuildSettingInfo(value = jobs), platform_common.TemplateVariableInfo({"FIX_JOBS": str(jobs)})]

fix_jobs = rule(implementation = _jobs, build_setting = config.int(flag = True))

def _cache(ctx):
    value = ctx.build_setting_value
    return [BuildSettingInfo(value = value), platform_common.TemplateVariableInfo({"FIX_CACHE": value})]

fix_cache = rule(implementation = _cache, build_setting = config.string(flag = True))
