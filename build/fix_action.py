import argparse
import fcntl
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


def import_repository(cache, archive, handle):
    if len(handle) != 64 or any(c not in "0123456789abcdef" for c in handle):
        raise ValueError("expected a canonical handle")
    marker = cache / "imports" / handle
    if marker.exists():
        return
    with tempfile.TemporaryDirectory(prefix="import-", dir=cache) as temporary:
        subprocess.run([
            "tar", "--extract", "--file", archive, "--directory", temporary,
            "--no-same-owner", "--no-same-permissions",
            "--exclude=.fix/labels", "--exclude=.fix/trust", ".fix",
        ], check=True)
        source = Path(temporary) / ".fix"
        for directory, _, files in os.walk(source):
            destination = cache / ".fix" / Path(directory).relative_to(source)
            destination.mkdir(parents=True, exist_ok=True)
            for name in files:
                original = Path(directory) / name
                target = destination / name
                try:
                    if original.is_symlink():
                        target.symlink_to(original.readlink())
                    else:
                        os.link(original, target)
                except FileExistsError:
                    pass
    marker.touch()


def run(config, cache, lock):
    root = Path.cwd()
    fix = str((root / config["fix"]).resolve())
    toolchain = json.loads((root / config["toolchain"]).read_text())
    if not (cache / ".fix").exists():
        subprocess.run([fix, "-C", cache, "init"], check=True)
    (cache / "imports").mkdir(exist_ok=True)
    import_repository(cache, root / config["repository"], toolchain["bootstrap"])
    for archive, handle in config["repositories"]:
        import_repository(cache, root / archive, (root / handle).read_text().strip())
    with tempfile.TemporaryDirectory(prefix="action-", dir=cache) as temporary:
        work = Path(temporary)
        (work / ".fix").symlink_to(cache / ".fix", target_is_directory=True)
        (work / "tools").mkdir()
        for name, handle in toolchain.items():
            if not name or name in (".", "..") or "/" in name:
                raise ValueError("invalid toolchain name")
            (work / "tools" / name).write_text(handle + "\n")
        environment = dict(os.environ, FIX=fix, FIX_TRUST=str(root / config["trust"]),
                           FIX_JSON=toolchain["json"], FIX_CPIO=toolchain["cpio"],
                           FIX_BINDINGS=toolchain["bootstrap"], execroot=str(root),
                           repository=str(work))
        environment["PATH"] = str(Path(fix).parent / "commands/bin") + os.pathsep + environment.get("PATH", os.defpath)
        subprocess.run(["bash", root / config["script"]], cwd=work, env=environment,
                       pass_fds=(lock.fileno(),), check=True)
        for handle in config["stored"]:
            (cache / "imports" / (root / handle).read_text().strip()).touch()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("config", nargs="?")
    parser.add_argument("--cache", default="")
    parser.add_argument("--reset", action="store_true")
    args = parser.parse_args()
    if not args.reset and args.config is None:
        parser.error("expected an action configuration or --reset")
    config = json.loads(Path(args.config).read_text()) if args.config else {}
    default = Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache")) / "fix/bazel"
    cache = Path(config.get("cache") or args.cache or os.environ.get("FIX_CACHE") or default).expanduser().resolve()
    cache.mkdir(parents=True, exist_ok=True)
    with (cache / "lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        if args.reset:
            paths = [cache / ".fix", cache / "imports"]
            paths += list(cache.glob("action-*")) + list(cache.glob("import-*"))
            for path in paths:
                if path.is_symlink():
                    path.unlink()
                elif path.is_dir():
                    shutil.rmtree(path)
        else:
            run(config, cache, lock)


if __name__ == "__main__":
    main()
