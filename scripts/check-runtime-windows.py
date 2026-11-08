"""Check Windows tools and run the player from an installed resource layout."""
import argparse
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("runtime", type=Path)
    parser.add_argument("--test-executable", type=Path, required=True)
    args = parser.parse_args()
    runtime = args.runtime.resolve()
    assert (runtime / "licenses/README.txt").is_file()
    assert (runtime / "cacert.pem").is_file()
    # Remove developer tools from PATH: every helper and DLL must come from the bundle.
    windows = Path(os.environ["WINDIR"])
    environment = {**os.environ, "PATH": os.pathsep.join(map(str, [windows / "System32", windows]))}
    checked = 0
    for path in (runtime / "bin").iterdir():
        if path.suffix.lower() not in (".exe", ".dll"):
            continue
        with path.open("rb") as source:
            assert source.read(2) == b"MZ", path
            source.seek(0x3C)
            offset = struct.unpack("<I", source.read(4))[0]
            source.seek(offset)
            assert source.read(4) == b"PE\0\0", path
            assert struct.unpack("<H", source.read(2))[0] == 0x8664, f"Not Windows x64: {path}"
        checked += 1
    for command in (["mpv.exe", "--version"], ["deno.exe", "eval", "console.log('bundled Deno OK')"],
                    ["yt-dlp.exe", "--ignore-config", "--version"]):
        result = subprocess.run([str(runtime / "bin" / command[0]), *command[1:]],
                                env=environment, text=True, capture_output=True, timeout=60)
        assert result.returncode == 0, (command[0], result.stderr)
        assert result.stdout.strip(), (command[0], "No version output")
        print(result.stdout.splitlines()[0])
    with tempfile.TemporaryDirectory(prefix="Lofi Radio installed check ") as temporary:
        installed = Path(temporary)
        shutil.copytree(runtime, installed / "runtime")
        executable = installed / "radio-check.exe"
        shutil.copy2(args.test_executable, executable)
        subprocess.run([str(executable)], env=environment, check=True, timeout=60)
        subprocess.run([str(executable), "bundled_windows_player_controls", "--ignored", "--nocapture"],
                       env=environment, check=True, timeout=60)
    print(f"Verified {checked} Windows x64 PE files, packaged IPC, playback, volume and cleanup.")


if __name__ == "__main__":
    main()
