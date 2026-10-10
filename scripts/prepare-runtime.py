"""Bundle the installed Homebrew playback runtime and its corresponding sources."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]
RUNTIME = ROOT / "src-tauri/runtime"
YTDLP_VERSION = "2026.08.19"
YTDLP_SHA256 = "1fa6733c37ea6fb51c99ad8fe785e7b7e5f3246c9b980230329d4fb72ed8d4d6"


def output(*args):
    return subprocess.check_output(args, text=True).strip()


def download(url, destination, checksum=None, revision=None):
    destination.parent.mkdir(parents=True, exist_ok=True)
    if checksum and destination.is_file() and hashlib.sha256(destination.read_bytes()).hexdigest() == checksum:
        return
    if url.endswith(".git"):
        if not revision:
            raise RuntimeError(f"Missing source revision: {url}")
        with tempfile.TemporaryDirectory(prefix="lofi-source-") as checkout:
            subprocess.run(["git", "init", "--quiet", checkout], check=True)
            subprocess.run(["git", "-C", checkout, "fetch", "--quiet", "--depth=1", url, revision], check=True)
            assert output("git", "-C", checkout, "rev-parse", "FETCH_HEAD") == revision
            subprocess.run(["git", "-C", checkout, "archive", "--format=tar.gz",
                            "--output", str(destination), "FETCH_HEAD"], check=True)
        return
    if url.startswith("https://ftpmirror.gnu.org/"):
        url = "https://ftp.gnu.org/gnu/" + url.removeprefix("https://ftpmirror.gnu.org/").removeprefix("gnu/")
    subprocess.run([
        "curl", "--fail", "--location", "--silent", "--show-error", "--retry", "3",
        "--connect-timeout", "15", "--max-time", "180",
        "--output", str(destination), url,
    ], check=True)
    if checksum and hashlib.sha256(destination.read_bytes()).hexdigest() != checksum:
        destination.unlink()
        raise RuntimeError(f"Checksum mismatch: {url}")


def dependencies(path):
    return [line.strip().split(" (")[0]
            for line in output("otool", "-L", str(path)).splitlines()[1:]]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sources", action="store_true", help="Archive corresponding source code")
    args = parser.parse_args()
    prefix = Path(output("brew", "--prefix"))
    cellar = Path(output("brew", "--cellar"))
    shutil.rmtree(RUNTIME, ignore_errors=True)
    bins = RUNTIME / "bin"
    bins.mkdir(parents=True)
    copied = {}
    kegs = {}

    def record(path):
        relative = path.relative_to(cellar)
        name, version = relative.parts[:2]
        keg = cellar / name / version
        if name in kegs and kegs[name] != keg:
            raise RuntimeError(f"Mixed versions of {name}")
        kegs[name] = keg

    def resolve(dependency, source):
        if dependency.startswith("@loader_path/"):
            return (source.parent / dependency.removeprefix("@loader_path/")).resolve()
        if dependency.startswith("@rpath/"):
            lines = output("otool", "-l", str(source)).splitlines()
            for index, line in enumerate(lines):
                if line.strip() == "cmd LC_RPATH":
                    rpath = lines[index + 2].strip().removeprefix("path ").split(" (offset")[0]
                    rpath = rpath.replace("@loader_path", str(source.parent))
                    candidate = Path(rpath) / dependency.removeprefix("@rpath/")
                    if candidate.is_file():
                        return candidate.resolve()
            raise RuntimeError(f"Unresolved {dependency} in {source}")
        if dependency.startswith("@"):
            raise RuntimeError(f"Unsupported dependency {dependency} in {source}")
        return Path(dependency).resolve()

    def bundle(source, destination=None):
        source = source.resolve()
        if source in copied:
            return copied[source]
        destination = destination or bins / source.name
        if destination.exists():
            raise RuntimeError(f"Runtime filename collision: {destination}")
        record(source)
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
        destination.chmod(0o755)
        copied[source] = destination
        changes = []
        for dep in dependencies(source):
            if dep.startswith(("/usr/lib/", "/System/Library/")):
                continue
            target = bundle(resolve(dep, source))
            changes.extend(["-change", dep,
                            "@loader_path/" + os.path.relpath(target, destination.parent)])
        if source.suffix in (".dylib", ".so") or source.name == "Python":
            changes.extend(["-id", "@rpath/" + destination.name])
        if changes:
            subprocess.run(["install_name_tool", *changes, str(destination)], check=True,
                           stderr=subprocess.PIPE)
        subprocess.run(["codesign", "--force", "--sign", "-", str(destination)],
                       check=True, stderr=subprocess.PIPE)
        return destination

    bundle(Path(output("which", "mpv")), bins / "mpv")
    bundle(Path(output("which", "deno")), bins / "deno")
    python_prefix = Path(output("brew", "--prefix", "python@3.14"))
    python = python_prefix / "Frameworks/Python.framework/Versions/3.14"
    bundle(python / "Resources/Python.app/Contents/MacOS/Python", bins / "python3")
    stdlib = RUNTIME / "python/lib/python3.14"
    shutil.copytree(python / "lib/python3.14", stdlib, ignore=shutil.ignore_patterns(
        "__pycache__", "test", "tests", "site-packages", "idlelib", "tkinter", "turtledemo", "config-*",
    ))
    for extension in stdlib.rglob("*.so"):
        relative = extension.relative_to(stdlib)
        extension.unlink()
        bundle(python / "lib/python3.14" / relative, extension)
    shutil.copy2(prefix / "etc/ca-certificates/cert.pem", RUNTIME / "cacert.pem")
    kegs["ca-certificates"] = Path(output("brew", "--prefix", "ca-certificates")).resolve()
    download(f"https://github.com/yt-dlp/yt-dlp/releases/download/{YTDLP_VERSION}/yt-dlp",
             RUNTIME / "yt-dlp.zip", YTDLP_SHA256)
    wrapper = bins / "yt-dlp"
    wrapper.write_text('''#!/bin/sh
set -eu
bin=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
export PYTHONHOME="$bin/../python"
export SSL_CERT_FILE="$bin/../cacert.pem"
exec "$bin/python3" -s "$bin/../yt-dlp.zip" "$@"
''')
    wrapper.chmod(0o755)

    # Save the actual formula versions; never attach sources from a different installed version.
    ruby = '''require "formulary"
    result = ARGV.map do |filename|
      path = Pathname.new(filename)
      formula = Formulary.from_contents(path.basename(".rb").to_s, path, path.read)
      info = formula.to_hash
      info["resources"] = formula.resources.map do |r|
        {"name" => r.name, "url" => r.url, "sha256" => r.checksum.to_s,
         "revision" => r.specs[:revision]}
      end
      info
    end
    puts JSON.generate(result)
    '''
    formulae = json.loads(subprocess.check_output(
        ["brew", "ruby", "-e", ruby, *[str(keg / ".brew" / f"{name}.rb")
                                    for name, keg in sorted(kegs.items())]],
        text=True, env={**os.environ, "HOMEBREW_DEVELOPER": "1"},
    ))
    licenses = RUNTIME / "licenses"
    licenses.mkdir()
    source_dir = ROOT / "src-tauri/target/third-party-sources"
    if args.sources:
        source_dir.mkdir(parents=True, exist_ok=True)
    downloads = []
    homebrew_commit = None
    for formula in formulae:
        name = formula["name"]
        keg = kegs[name]
        installed_version = keg.name.rsplit("_", 1)[0]
        if installed_version != formula["versions"]["stable"]:
            raise RuntimeError(f"Source metadata does not match {name} {keg.name}")
        notice_dir = licenses / name
        notice_dir.mkdir()
        shutil.copy2(keg / ".brew" / f"{name}.rb", notice_dir / f"{name}.rb")
        shutil.copy2(keg / "INSTALL_RECEIPT.json", notice_dir / "INSTALL_RECEIPT.json")
        (notice_dir / "formula.json").write_text(json.dumps(formula, indent=2) + "\n")
        for path in keg.rglob("*"):
            if path.is_file() and path.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "COPYRIGHT", "NOTICE")):
                target = notice_dir / path.relative_to(keg)
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(path, target)
                target.chmod(0o644)
        source = formula["urls"]["stable"]
        suffix = Path(source["url"].split("?", 1)[0]).suffixes
        archive_name = "source.tar.gz" if source["url"].endswith(".git") else "source" + "".join(suffix[-2:])
        downloads.append((source["url"], source_dir / name / archive_name, source["checksum"], source.get("revision")))
        for index, patch in enumerate(formula.get("patches", [])):
            if "url" in patch:
                downloads.append((patch["url"], source_dir / name / f"patch-{index}.patch", patch["sha256"]))
            elif "file" in patch and args.sources:
                if homebrew_commit is None:
                    homebrew_commit = output(
                        "git", "ls-remote", "https://github.com/Homebrew/homebrew-core.git", "HEAD",
                    ).split()[0]
                downloads.append((
                    f"https://raw.githubusercontent.com/Homebrew/homebrew-core/{homebrew_commit}/{patch['file']}",
                    source_dir / name / patch["file"], None,
                ))
        # Some formulas require additional sources (e.g. FFmpeg's external codec resources).
        for resource in formula.get("resources", []):
            url = resource["url"]
            filename = resource["name"] + ".tar.gz" if url.endswith(".git") else Path(url.split("?")[0]).name
            downloads.append((url, source_dir / name / "resources" / filename,
                              resource.get("sha256"), resource.get("revision")))

    for filename in ("LICENSE", "THIRD_PARTY_LICENSES.txt"):
        download(f"https://raw.githubusercontent.com/yt-dlp/yt-dlp/{YTDLP_VERSION}/{filename}",
                 licenses / f"yt-dlp-{filename}")
    (licenses / "README.txt").write_text(
        "Lofi Radio is MIT licensed. Bundled tools retain their own licenses.\n"
        "mpv and some linked libraries are GPL/LGPL licensed. They run as a separate process.\n"
        f"yt-dlp {YTDLP_VERSION}: official zipimport executable, including bundled EJS scripts.\n"
        "Corresponding sources, patches and build recipes are in the third-party-sources asset\n"
        "of the same release: https://github.com/cnlutong/Lofi-Radio/releases\n"
        "Rebuild Homebrew dependencies using the included formulas, then run prepare-runtime.py.\n"
        "Binary modifications only relocate library paths and replace signatures with ad-hoc signatures.\n"
    )
    shutil.copy2(ROOT / "LICENSE", licenses / "Lofi-Radio-MIT.txt")
    if args.sources:
        downloads.append((f"https://github.com/yt-dlp/yt-dlp/releases/download/{YTDLP_VERSION}/yt-dlp.tar.gz",
                          source_dir / "yt-dlp/source.tar.gz",
                          "072aad4f2a7604e92155f61a275a4752dc64046c8f6d90df3710525d94cd37c1"))
        def fetch(item):
            url, target, *_ = item
            print(f"Downloading source: {target.parent.name}/{target.name}", flush=True)
            download(*item)
        with ThreadPoolExecutor(max_workers=6) as pool:
            list(pool.map(fetch, downloads))
        shutil.rmtree(source_dir / "licenses", ignore_errors=True)
        shutil.copytree(licenses, source_dir / "licenses")
        shutil.copy2(Path(__file__), source_dir / "prepare-runtime.py")
        archive = ROOT / "src-tauri/target/third-party-sources.tar.gz"
        with tarfile.open(archive, "w:gz") as tar:
            tar.add(source_dir, arcname="third-party-sources")
        print(f"Source archive: {archive}")
    print(f"Bundled {len(copied)} Mach-O files from {len(kegs)} formulas into {RUNTIME}")


if __name__ == "__main__":
    main()
