"""Publish only the complete draft prepared by cargo xtask deploy; never build or sign on a runner."""

import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import xml.etree.ElementTree as ET


REPOSITORY = "sventmueller/dango"


def run(*args):
    return subprocess.check_output(args, text=True).strip()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def assets(version):
    return {f"Dango-{version}.dmg", "Dango.dmg", f"Dango-{version}.md", "appcast.xml", "dango.rb"}


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for block in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def verify_files(directory, manifest, version, commit, fingerprint):
    require(manifest["version"] == version, "manifest version differs from VERSION")
    require(manifest["commit"] == commit, "manifest commit differs from the pushed commit")
    require(manifest["fingerprint"] == fingerprint, "manifest does not describe the pushed source")
    require(set(manifest["files"]) == assets(version), "manifest has missing or unexpected assets")
    for name, digest in manifest["files"].items():
        path = directory / name
        require(path.stat().st_size > 0, f"empty asset: {name}")
        require(sha256(path) == digest, f"checksum mismatch: {name}")
    require(manifest["files"]["Dango.dmg"] == manifest["files"][f"Dango-{version}.dmg"], "download alias differs from the update image")
    sparkle = "{http://www.andymatuschak.org/xml-namespaces/sparkle}"
    item = ET.parse(directory / "appcast.xml").find("./channel/item")
    require(item is not None, "the update feed has no release")
    require(item.findtext(f"{sparkle}version") == version, "the update feed has another version")
    enclosure = item.find("enclosure")
    require(enclosure is not None, "the update feed has no image")
    expected_url = f"https://github.com/{REPOSITORY}/releases/download/v{version}/Dango-{version}.dmg"
    require(enclosure.get("url") == expected_url, "the update feed points at another image")
    signature = enclosure.get(f"{sparkle}edSignature", "")
    require(signature and not signature.startswith("DRY-RUN"), "the update feed is unsigned")
    require(enclosure.get("length") == str((directory / "Dango.dmg").stat().st_size), "the update feed has the wrong image length")


def main():
    event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text())
    commit = os.environ["GITHUB_SHA"]
    require(os.environ["GITHUB_REPOSITORY"] == REPOSITORY, "unexpected repository")
    require(os.environ["GITHUB_REF"] == "refs/heads/main", "releases must come from main")
    require(run("git", "rev-parse", "HEAD") == commit, "checkout differs from the workflow commit")
    version = Path("VERSION").read_text().strip()
    require(re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version), "invalid VERSION")
    before = event.get("before")
    if not before or set(before) == {"0"}:
        # A manual retry is still tied to the original VERSION-only release commit.
        before = run("git", "rev-parse", "HEAD^")
    previous = run("git", "show", f"{before}:VERSION")
    if previous == version:
        print("VERSION unchanged; no release")
        return
    require(tuple(map(int, version.split("."))) > tuple(map(int, previous.split("."))), "VERSION must increase")
    parent = run("git", "rev-parse", "HEAD^")
    require(run("git", "diff", "--name-only", parent, commit) == "VERSION", "the release commit must change only VERSION")
    tag = f"v{version}"
    require(run("git", "rev-parse", f"refs/tags/{tag}^{{commit}}") == commit, "version tag differs from the release commit")
    # Same fingerprint as xtask: the HEAD tree with VERSION removed, using a private temporary index.
    with tempfile.TemporaryDirectory() as temporary:
        env = dict(os.environ, GIT_INDEX_FILE=str(Path(temporary) / "index"))
        for args in (("read-tree", "HEAD"), ("rm", "--cached", "--quiet", "--", "VERSION")):
            subprocess.run(["git", *args], check=True, env=env)
        fingerprint = subprocess.check_output(["git", "write-tree"], env=env, text=True).strip()
    seals = run("git", "ls-remote", "origin", f"refs/ci-seals/*/{fingerprint}")
    references = {line.split()[1] for line in seals.splitlines()}
    require(f"refs/ci-seals/green/{fingerprint}" in references, "the pushed source has no green CI seal")
    require(f"refs/ci-seals/red/{fingerprint}" not in references, "the pushed source has a red CI seal")
    release = json.loads(run("gh", "release", "view", tag, "--repo", REPOSITORY, "--json", "isDraft,assets"))
    expected = assets(version) | {"deployment.json"}
    require({asset["name"] for asset in release["assets"]} == expected, "the draft has missing or unexpected assets")
    with tempfile.TemporaryDirectory() as temporary:
        directory = Path(temporary)
        run("gh", "release", "download", tag, "--repo", REPOSITORY, "--dir", temporary)
        manifest = json.loads((directory / "deployment.json").read_text())
        require(manifest["source_commit"] == parent, "the assets were built from another source commit")
        verify_files(directory, manifest, version, commit, fingerprint)
    if not release["isDraft"]:
        print(f"{tag} is already published and verified")
        return
    # Never move latest backwards when an older workflow is retried after a newer deploy.
    run("git", "fetch", "--quiet", "origin", "main")
    require(run("git", "show", "FETCH_HEAD:VERSION") == version, "a newer version was deployed; refusing to move latest backwards")
    run("gh", "release", "edit", tag, "--repo", REPOSITORY, "--target", commit, "--draft=false", "--latest")
    print(f"Published https://github.com/{REPOSITORY}/releases/tag/{tag}")


if __name__ == "__main__":
    main()
