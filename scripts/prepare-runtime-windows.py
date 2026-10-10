"""Bundle native Windows playback tools, licenses and corresponding MSYS2 sources."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import time
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[1]
RUNTIME = ROOT / "src-tauri/runtime"
YTDLP_VERSION = "2026.08.19"
DENO_VERSION = "2.9.7"


def download(url, path, checksum=None):
    path.parent.mkdir(parents=True, exist_ok=True)
    for attempt in range(3):
        try:
            if not path.exists():
                with urllib.request.urlopen(url, timeout=90) as response, path.open("wb") as target:
                    shutil.copyfileobj(response, target)
            with path.open("rb") as source:
                digest = hashlib.file_digest(source, "sha256").hexdigest()
            if checksum and digest != checksum:
                raise ValueError(f"Checksum mismatch: {path.name}")
            return digest
        except Exception:
            path.unlink(missing_ok=True)
            if attempt == 2:
                raise
            time.sleep(2)


def fields(text):
    return {name: value.strip().splitlines()
            for name, value in re.findall(r"%([^%]+)%\n(.*?)(?=\n%|\Z)", text, re.S)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--msys-root", type=Path, default=Path("C:/msys64"))
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if args.check:
        record = fields("%NAME%\nmingw-w64-ucrt-x86_64-mpv\n\n%VERSION%\n0.41.0-8\n\n%FILES%\nucrt64/bin/mpv.exe\n")
        assert record["NAME"] == ["mingw-w64-ucrt-x86_64-mpv"]
        assert record["VERSION"][0] == "0.41.0-8"
        assert record["FILES"][0] == "ucrt64/bin/mpv.exe"
        print("MSYS2 metadata check passed")
        return
    assert os.name == "nt", "Run preparation on a native Windows runner"
    msys = args.msys_root.resolve()
    binaries = msys / "ucrt64/bin"
    objdump = binaries / "objdump.exe"
    assert objdump.is_file() and (binaries / "mpv.exe").is_file()
    shutil.rmtree(RUNTIME, ignore_errors=True)
    destination = RUNTIME / "bin"
    destination.mkdir(parents=True)
    notices = RUNTIME / "licenses"
    notices.mkdir()
    source_dir = ROOT / "src-tauri/target/third-party-sources-windows"
    source_dir.mkdir(parents=True, exist_ok=True)
    records, owners = {}, {}
    for directory in (msys / "var/lib/pacman/local").iterdir():
        if not (directory / "desc").is_file():
            continue
        record = fields((directory / "desc").read_text())
        name = record["NAME"][0]
        records[name] = (directory, record)
        for filename in fields((directory / "files").read_text()).get("FILES", []):
            if filename:
                owners[filename.lower()] = name
    available = {path.name.lower(): path for path in binaries.glob("*.dll")}
    system_dlls = {p.name.lower() for p in (Path(os.environ["WINDIR"]) / "System32").glob("*.dll")}
    pending, copied, packages = [binaries / "mpv.exe"], set(), set()
    while pending:
        binary = pending.pop()
        if binary.name.lower() in copied:
            continue
        copied.add(binary.name.lower())
        shutil.copy2(binary, destination / binary.name)
        package = owners[binary.relative_to(msys).as_posix().lower()]
        packages.add(package)
        imports = subprocess.check_output([str(objdump), "-p", str(binary)], text=True)
        for name in re.findall(r"DLL Name:\s*(\S+)", imports):
            if name.lower() in available:
                pending.append(available[name.lower()])
            else:
                assert name.lower() in system_dlls or name.lower().startswith(("api-ms-", "ext-ms-")), f"Missing DLL: {name}"
    downloads = ROOT / "src-tauri/target/windows-downloads"
    ytdlp = downloads / "yt-dlp.exe"
    ytdlp_sha = download(f"https://github.com/yt-dlp/yt-dlp/releases/download/{YTDLP_VERSION}/yt-dlp.exe", ytdlp,
                        "66674953fe251b89f4d08c5f0e35e0728679bd67ab3d7d05c0562af101dd3e7a")
    shutil.copy2(ytdlp, destination / "yt-dlp.exe")
    deno = downloads / "deno.zip"
    deno_sha = download(f"https://github.com/denoland/deno/releases/download/v{DENO_VERSION}/deno-x86_64-pc-windows-msvc.zip", deno,
                       "a0c3101b4158d1dfb7d6a78a7bf0f3de80c96bb423c152beec8beb22786f2238")
    with zipfile.ZipFile(deno) as archive, (destination / "deno.exe").open("wb") as target:
        with archive.open("deno.exe") as executable:
            shutil.copyfileobj(executable, target)
    manifest = {"yt-dlp": {"version": YTDLP_VERSION, "sha256": ytdlp_sha},
                "deno": {"version": DENO_VERSION, "archive_sha256": deno_sha}, "msys2": {}}

    def package_sources(name):
        directory, record = records[name]
        version = record["VERSION"][0]
        base = record.get("BASE", [name.replace("mingw-w64-ucrt-x86_64-", "mingw-w64-")])[0]
        notice = notices / name
        notice.mkdir()
        shutil.copy2(directory / "desc", notice / "package.txt")
        for filename, owner in owners.items():
            if owner != name or not filename.startswith("ucrt64/share/"):
                continue
            if "/licenses/" in filename or Path(filename).name.startswith(("license", "copying", "copyright", "notice")):
                path = msys / filename
                if path.is_file():
                    target = notice / Path(filename).relative_to("ucrt64/share")
                    target.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(path, target)
        # MSYS2 all-source archives include upstream source, patches and the exact PKGBUILD.
        filename = f"{base}-{version.split(':')[-1]}.src.tar.zst"
        url = f"https://repo.msys2.org/mingw/sources/{filename}"
        path = source_dir / filename
        digest = download(url, path)
        download(url + ".sig", path.with_suffix(path.suffix + ".sig"))
        subprocess.run([str(msys / "usr/bin/bash.exe"), "-lc",
                        'pacman-key --verify "$1"', "--", path.as_posix() + ".sig"], check=True, capture_output=True)
        print(f"Verified source: {filename}", flush=True)
        return name, {"version": version, "source": url, "sha256": digest}

    with ThreadPoolExecutor(max_workers=4) as pool:
        manifest["msys2"] = dict(pool.map(package_sources, sorted(packages)))
    common = msys / "usr/share/licenses/common"
    assert common.is_dir(), "Missing standard license texts"
    shutil.copytree(common, notices / "common")
    for filename in ("LICENSE", "THIRD_PARTY_LICENSES.txt"):
        download(f"https://raw.githubusercontent.com/yt-dlp/yt-dlp/{YTDLP_VERSION}/{filename}", notices / f"yt-dlp-{filename}")
    download(f"https://raw.githubusercontent.com/denoland/deno/v{DENO_VERSION}/LICENSE.md", notices / "deno-LICENSE.md")
    download(f"https://github.com/yt-dlp/yt-dlp/releases/download/{YTDLP_VERSION}/yt-dlp.tar.gz", source_dir / "yt-dlp.tar.gz",
             "072aad4f2a7604e92155f61a275a4752dc64046c8f6d90df3710525d94cd37c1")
    shutil.copy2(ROOT / "LICENSE", notices / "Lofi-Radio-MIT.txt")
    (notices / "versions.json").write_text(json.dumps(manifest, indent=2) + "\n")
    (notices / "README.txt").write_text(
        "Lofi Radio code is MIT licensed. Bundled tools retain their own licenses.\n"
        "mpv and some dependencies are GPL/LGPL; see package records and common license texts.\n"
        "Unmodified native MSYS2 UCRT64 binaries; no MSYS2 shell is needed at runtime.\n"
        "Corresponding all-source archives, signatures, patches and PKGBUILDs are included\n"
        "in third-party-sources-windows-x64.tar.gz in the same GitHub release.\n"
        "Rebuild each package with MSYS2 makepkg-mingw, then run prepare-runtime-windows.py.\n"
        "yt-dlp: official standalone Windows executable, including Python and EJS scripts.\n"
        "Deno source: https://github.com/denoland/deno/tree/v" + DENO_VERSION + "\n"
    )
    shutil.rmtree(source_dir / "licenses", ignore_errors=True)
    shutil.copytree(notices, source_dir / "licenses")
    shutil.copy2(Path(__file__), source_dir / "prepare-runtime-windows.py")
    archive_path = ROOT / "src-tauri/target/third-party-sources-windows-x64.tar.gz"
    with tarfile.open(archive_path, "w:gz") as archive:
        archive.add(source_dir, arcname="third-party-sources-windows-x64")
    print(f"Bundled {len(copied)} native PE files from {len(packages)} MSYS2 packages: {RUNTIME}")


if __name__ == "__main__":
    main()
