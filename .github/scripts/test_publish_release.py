"""Release gates against real temporary git repositories; only GitHub's API is replaced."""

from contextlib import redirect_stdout
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch


spec = importlib.util.spec_from_file_location("publish", Path(__file__).with_name("publish-release.py"))
publish = importlib.util.module_from_spec(spec)
spec.loader.exec_module(publish)


class PublicationTests(unittest.TestCase):
    def setUp(self):
        environment = patch.dict(os.environ, {"GIT_CONFIG_GLOBAL": "/dev/null", "GIT_CONFIG_NOSYSTEM": "1"})
        environment.start()
        self.addCleanup(environment.stop)
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        directory = Path(self.temporary.name)
        self.repo = directory / "repo"
        self.repo.mkdir()
        self.original_cwd = Path.cwd()
        os.chdir(self.repo)
        self.addCleanup(os.chdir, self.original_cwd)
        self.git("init", "--quiet", "-b", "main")
        self.git("config", "user.name", "Test")
        self.git("config", "user.email", "test@example.invalid")
        self.git("config", "commit.gpgsign", "false")
        Path("VERSION").write_text("0.1.2\n")
        Path("source.txt").write_text("source\n")
        self.git("add", ".")
        self.git("commit", "--quiet", "-m", "Source")
        self.source = self.git("rev-parse", "HEAD")
        # Independently construct the expected tree by listing all entries except VERSION.
        entries = [line for line in self.git("ls-tree", "HEAD").splitlines() if not line.endswith("\tVERSION")]
        self.fp = subprocess.check_output(["git", "mktree"], input="\n".join(entries) + "\n", text=True).strip()
        Path("VERSION").write_text("0.1.3\n")
        self.git("add", "VERSION")
        self.git("commit", "--quiet", "-m", "Release Dango 0.1.3")
        self.commit = self.git("rev-parse", "HEAD")
        self.git("-c", "tag.gpgsign=false", "tag", "v0.1.3")
        self.green = f"refs/ci-seals/green/{self.fp}"
        self.git("update-ref", self.green, self.commit)
        self.origin = directory / "origin.git"
        self.git("init", "--quiet", "--bare", str(self.origin))
        self.git("remote", "add", "origin", str(self.origin))
        self.git("push", "--quiet", "--atomic", "origin", "main", "refs/tags/v0.1.3", self.green)
        self.asset_dir = directory / "assets"
        self.asset_dir.mkdir()
        for name in publish.assets("0.1.3"):
            (self.asset_dir / name).write_text("release asset\n")
        size = (self.asset_dir / "Dango.dmg").stat().st_size
        (self.asset_dir / "appcast.xml").write_text(f'''<rss xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle"><channel><item>
<sparkle:version>0.1.3</sparkle:version>
<enclosure url="https://github.com/sventmueller/dango/releases/download/v0.1.3/Dango-0.1.3.dmg"
sparkle:edSignature="signed" length="{size}"/></item></channel></rss>''')
        self.manifest = {
            "version": "0.1.3", "commit": self.commit, "source_commit": self.source, "fingerprint": self.fp,
            "files": {name: hashlib.sha256((self.asset_dir / name).read_bytes()).hexdigest() for name in publish.assets("0.1.3")},
        }
        self.save_manifest()
        self.release = {"isDraft": True, "assets": [{"name": name} for name in (*publish.assets("0.1.3"), "deployment.json")]}
        self.event_file = directory / "event.json"
        self.event_file.write_text(json.dumps({"before": self.source}))
        self.environment = {
            "GITHUB_EVENT_PATH": str(self.event_file), "GITHUB_SHA": self.commit,
            "GITHUB_REPOSITORY": publish.REPOSITORY, "GITHUB_REF": "refs/heads/main",
        }
        self.github_calls = []
        self.real_run = publish.run

    def git(self, *args):
        return subprocess.check_output(["git", *args], text=True, stderr=subprocess.PIPE).strip()

    def save_manifest(self):
        (self.asset_dir / "deployment.json").write_text(json.dumps(self.manifest))

    def command(self, *args):
        if args[0] != "gh":
            return self.real_run(*args)
        self.github_calls.append(args)
        if args[2] == "view":
            return json.dumps(self.release)
        if args[2] == "download":
            destination = Path(args[args.index("--dir") + 1])
            for file in self.asset_dir.iterdir():
                shutil.copy(file, destination / file.name)
        return ""

    def execute(self):
        with patch.dict(os.environ, self.environment), patch.object(publish, "run", side_effect=self.command), redirect_stdout(io.StringIO()):
            publish.main()

    def assert_refused(self, message):
        with self.assertRaisesRegex(ValueError, message):
            self.execute()
        self.assertFalse(any(call[2] == "edit" for call in self.github_calls), "a failed gate must never publish")

    def test_complete_deployment_publishes_the_exact_version(self):
        self.execute()
        self.assertEqual(self.github_calls[-1], ("gh", "release", "edit", "v0.1.3", "--repo", publish.REPOSITORY, "--target", self.commit, "--draft=false", "--latest"))

    def test_unchanged_version_creates_no_release(self):
        self.event_file.write_text(json.dumps({"before": self.commit}))
        self.execute()
        self.assertEqual(self.github_calls, [])

    def test_missing_assets_are_refused(self):
        self.release["assets"].pop()
        self.assert_refused("missing or unexpected assets")

    def test_tampered_image_is_refused(self):
        (self.asset_dir / "Dango.dmg").write_text("changed after upload")
        self.assert_refused("checksum mismatch")

    def test_manifest_from_another_commit_is_refused(self):
        self.manifest["commit"] = self.source
        self.save_manifest()
        self.assert_refused("manifest commit differs")

    def test_dry_run_assets_are_refused_even_with_matching_checksums(self):
        feed = self.asset_dir / "appcast.xml"
        feed.write_text(feed.read_text().replace('edSignature="signed"', 'edSignature="DRY-RUN-NOT-SIGNED"'))
        self.manifest["files"]["appcast.xml"] = hashlib.sha256(feed.read_bytes()).hexdigest()
        self.save_manifest()
        self.assert_refused("update feed is unsigned")

    def test_a_tag_on_another_commit_is_refused(self):
        self.git("update-ref", "refs/tags/v0.1.3", self.source)
        self.assert_refused("version tag differs")

    def test_seals_do_not_gate_publication(self):
        self.git("--git-dir", str(self.origin), "update-ref", "-d", self.green)
        self.git("--git-dir", str(self.origin), "update-ref", f"refs/ci-seals/red/{self.fp}", self.commit)
        self.execute()
        self.assertEqual(self.github_calls[-1][2], "edit")

    def test_manual_retry_verifies_an_already_published_release(self):
        self.event_file.write_text("{}")
        self.release["isDraft"] = False
        self.execute()
        self.assertFalse(any(call[2] == "edit" for call in self.github_calls))

    def test_older_retry_cannot_move_latest_backwards(self):
        Path("VERSION").write_text("0.1.4\n")
        self.git("add", "VERSION")
        self.git("commit", "--quiet", "-m", "Next release")
        self.git("push", "--quiet", "origin", "main")
        self.git("checkout", "--quiet", self.commit)
        self.assert_refused("newer version was deployed")


if __name__ == "__main__":
    unittest.main()
