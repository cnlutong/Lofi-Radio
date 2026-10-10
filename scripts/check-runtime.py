"""Check a packaged runtime without relying on Homebrew or user configuration."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("runtime", type=Path)
    parser.add_argument("--maximum-macos", default="15.0")
    parser.add_argument("--test-executable", type=Path)
    parser.add_argument("--live", action="store_true")
    args = parser.parse_args()
    runtime = args.runtime.resolve()
    environment = {**os.environ, "PATH": "/usr/bin:/bin:/usr/sbin:/sbin"}
    for name in ("DYLD_LIBRARY_PATH", "DYLD_FALLBACK_LIBRARY_PATH", "PYTHONPATH", "PYTHONHOME"):
        environment.pop(name, None)
    assert (runtime / "licenses/README.txt").is_file(), "Missing third-party notices"
    maximum = tuple(map(int, args.maximum_macos.split(".")))
    checked = 0
    for path in runtime.rglob("*"):
        if not path.is_file() or "Mach-O" not in subprocess.check_output(["file", str(path)], text=True):
            continue
        linked = subprocess.check_output(["otool", "-L", str(path)], text=True)
        for line in linked.splitlines()[1:]:
            dep = line.strip().split(" (")[0]
            if dep.startswith("@loader_path/"):
                assert (path.parent / dep.removeprefix("@loader_path/")).is_file(), (path, dep)
            else:
                assert dep.startswith(("/usr/lib/", "/System/Library/", "@rpath/")), (path, dep)
        build = subprocess.check_output(["vtool", "-show-build", str(path)], text=True)
        versions = re.findall(r"\bminos\s+(\d+\.\d+)", build)
        if not versions:
            versions = re.findall(r"(?m)^\s*version\s+(\d+\.\d+)", build)
        assert versions, f"No minimum OS recorded: {path}"
        assert all(tuple(map(int, v.split("."))) <= maximum for v in versions), (path, versions)
        subprocess.run(["codesign", "--verify", "--strict", str(path)], check=True)
        checked += 1
    for command in (["mpv", "--version"], ["deno", "eval", "console.log('bundled Deno OK')"],
                    ["yt-dlp", "--ignore-config", "--version"]):
        result = subprocess.run([str(runtime / "bin" / command[0]), *command[1:]],
                                env=environment, text=True, capture_output=True, timeout=30)
        assert result.returncode == 0, result.stderr
        print(result.stdout.splitlines()[0])
    python_environment = {**environment, "PYTHONHOME": str(runtime / "python"),
                          "SSL_CERT_FILE": str(runtime / "cacert.pem")}
    subprocess.run([str(runtime / "bin/python3"), "-s", "-c",
                    "import ssl, sqlite3, bz2, lzma; assert ssl.create_default_context().cert_store_stats()['x509_ca'] > 0"],
                   env=python_environment, check=True, timeout=30)
    if args.test_executable:
        # Run the existing Rust checks from an app layout so binary() selects bundled tools.
        with tempfile.TemporaryDirectory(prefix="Lofi Radio check ") as temporary:
            contents = Path(temporary) / "Lofi Radio.app/Contents"
            macos = contents / "MacOS"
            macos.mkdir(parents=True)
            (contents / "Resources").mkdir()
            (contents / "Resources/runtime").symlink_to(runtime, target_is_directory=True)
            executable = macos / "radio-check"
            shutil.copy2(args.test_executable, executable)
            command = [str(executable)]
            if args.live:
                command += ["live_stream_controls", "--ignored", "--nocapture"]
            subprocess.run(command, env=environment, check=True)
    print(f"Verified {checked} Mach-O files; no Homebrew library paths; macOS <= {args.maximum_macos}.")


if __name__ == "__main__":
    main()
