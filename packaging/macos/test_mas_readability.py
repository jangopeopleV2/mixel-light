#!/usr/bin/env python3
"""Six inert readability regression checks; all subprocess calls are faked."""
import argparse
from contextlib import ExitStack, contextmanager
import datetime as dt
import os
from pathlib import Path
import shutil
import stat
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

HELPER = Path(__file__).with_name("mas_package.py")


def load_helper(path):
    module = types.ModuleType("mas_readability_under_test")
    module.__file__ = str(path)
    exec(compile(path.read_bytes(), str(path), "exec"), module.__dict__)
    return module


@contextmanager
def caller_umask(value):
    previous = os.umask(value)
    try:
        yield
    finally:
        os.umask(previous)


def fake_sign(subject, child_umask=-1):
    """Model a signer creating its signature under the requested child mask."""
    previous = os.umask(child_umask) if child_umask != -1 else None
    try:
        signature = subject / "Contents/_CodeSignature"
        signature.mkdir()
        (signature / "CodeResources").write_bytes(b"fake sealed resource table")
    finally:
        if previous is not None:
            os.umask(previous)


class ReadabilityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.mas = load_helper(HELPER)

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def bundle(self, name="Fixture.app"):
        app = self.root / name
        executable = app / "Contents/MacOS/Fixture"
        executable.parent.mkdir(parents=True)
        executable.write_bytes(b"fake executable")
        signature = app / "Contents/_CodeSignature/CodeResources"
        signature.parent.mkdir()
        signature.write_bytes(b"fake signature")
        for path in (app, *app.rglob("*")):
            path.chmod(0o755 if path.is_dir() or path == executable else 0o644)
        return app

    def test_private_parent_signer_mask_and_private_log(self):
        first, second = self.root / "Inherited.app", self.root / "Public.app"
        (first / "Contents").mkdir(parents=True)
        (second / "Contents").mkdir(parents=True)
        calls = []
        def fake_process(argv, **kwargs):
            calls.append(kwargs["umask"])
            fake_sign(Path(argv[-1]), kwargs["umask"])
            return types.SimpleNamespace(returncode=0, stdout=b"private signer detail", stderr=b"")
        log = self.root / "private-signing.log"
        with caller_umask(0o077), patch.object(self.mas.subprocess, "run", side_effect=fake_process):
            self.mas.run(["fake-codesign", str(first)])
            self.mas.run(["fake-codesign", str(second)], umask=0o022, log=log)
            private_probe = self.root / "private-after-child"
            private_probe.write_bytes(b"still private")
        self.assertEqual(calls, [-1, 0o022])
        self.assertEqual(stat.S_IMODE((first / "Contents/_CodeSignature/CodeResources").stat().st_mode), 0o600)
        self.assertEqual(stat.S_IMODE((second / "Contents/_CodeSignature/CodeResources").stat().st_mode), 0o644)
        self.assertEqual(stat.S_IMODE((second / "Contents/_CodeSignature").stat().st_mode), 0o755)
        self.assertEqual(stat.S_IMODE(log.stat().st_mode), 0o600)
        self.assertEqual(stat.S_IMODE(private_probe.stat().st_mode), 0o600)

    def test_signature_file_0600_is_rejected(self):
        app = self.bundle()
        (app / "Contents/_CodeSignature/CodeResources").chmod(0o600)
        with self.assertRaisesRegex(self.mas.Failure, "not publicly readable"):
            self.mas.verify_public_bundle_readability(app)

    def test_nested_directory_0700_is_rejected(self):
        app = self.bundle()
        (app / "Contents/_CodeSignature").chmod(0o700)
        with self.assertRaisesRegex(self.mas.Failure, "not publicly readable and traversable"):
            self.mas.verify_public_bundle_readability(app)

    def test_app_root_0700_is_rejected(self):
        app = self.bundle()
        app.chmod(0o700)
        with self.assertRaisesRegex(self.mas.Failure, "traversable: \\."):
            self.mas.verify_public_bundle_readability(app)

    def test_public_bundle_is_accepted_without_mutation(self):
        app = self.bundle()
        def snapshot():
            return [(str(p.relative_to(app)), stat.S_IMODE(p.lstat().st_mode),
                     None if p.is_dir() else p.read_bytes()) for p in (app, *sorted(app.rglob("*")))]
        before = snapshot()
        self.mas.verify_public_bundle_readability(app)
        self.assertEqual(snapshot(), before)

    def package_fixture(self, directory, poison_stage=False, poison_expansion=False):
        """Run the real package control flow with filesystem-only tool doubles."""
        directory.mkdir()
        root = directory / "mixel-photo"
        icon = root / "assets/app-icon/photocraft.icns"
        icon.parent.mkdir(parents=True)
        icon.write_bytes(b"fake icon")
        (root / "packaging/macos").mkdir(parents=True)
        target = directory / "isolated-target"
        binary = directory / "compiled-binary"
        binary.write_bytes(b"fake compiled binary")
        receipt_path = directory / "build-receipt.json"
        receipt_path.write_text("{}")
        profile, privacy = directory / "profile", directory / "privacy"
        profile.write_bytes(b"fake profile")
        privacy.write_bytes(b"fake privacy")
        receipt = {"binaries": [{"file": {"path": str(binary)}}], "sourceCommit": "a" * 40,
                   "source": {"sha256": "fixture-source"}, "externalFonts": None}
        signing = {"policy": {"expiresAtUtc": (dt.datetime.now(dt.timezone.utc) + dt.timedelta(days=1)).isoformat()},
                   "identity": "a" * 40, "installer": "3rd Party Mac Developer Installer: Fixture",
                   "profilePath": profile, "profileProof": self.mas.file_proof(profile), "metadataProof": {"path": "not-read"}}
        events, staged = [], None
        self.events = events
        def fake_process(argv, **kwargs):
            nonlocal staged
            events.append((Path(argv[0]).name, tuple(argv[1:])))
            stdout = b""
            if argv[0] == self.mas.TOOLS["lipo"]:
                executable = Path(argv[argv.index("-output") + 1])
                executable.write_bytes(binary.read_bytes())
                staged = executable.parents[2]
            elif argv[0] == self.mas.TOOLS["codesign"] and "--force" in argv:
                self.assertEqual(kwargs["umask"], 0o022)
                subject = Path(argv[-1])
                if subject.is_dir():
                    fake_sign(subject, kwargs["umask"])
                    if poison_stage:
                        (subject / "Contents/_CodeSignature/CodeResources").chmod(0o600)
            elif argv[0] == self.mas.TOOLS["productbuild"]:
                Path(argv[-1]).write_bytes(b"fake signed package")
            elif argv[0] == self.mas.TOOLS["pkgutil"] and "--expand-full" in argv:
                destination = Path(argv[-1])
                destination.mkdir()
                expanded_app = destination / staged.name
                shutil.copytree(staged, expanded_app)
                if poison_expansion:
                    expanded_app.chmod(0o700)
            elif argv[0] == self.mas.TOOLS["pkgutil"] and "--check-signature" in argv:
                stdout = signing["installer"].encode()
            return types.SimpleNamespace(returncode=0, stdout=stdout, stderr=b"")
        values = {"build_environment": ({"PHOTOCRAFT_BUILD_SHA": "a" * 40}, target),
                  "load_version": "1.2.3", "distribution_inputs": signing,
                  "load_privacy": (privacy, self.mas.file_proof(privacy)),
                  "checked_build_receipt": receipt, "copy_legal": ["LICENSE"],
                  "generated_info": {}, "desired_entitlements": {}, "source_catalogue": receipt["source"],
                  "external_fonts": None, "command_text": "arm64", "verify_app": None,
                  "verify_app_private_api": None}
        with ExitStack() as stack:
            for name, value in values.items():
                stack.enter_context(patch.object(self.mas, name, return_value=value))
            stack.enter_context(patch.object(self.mas.subprocess, "run", side_effect=fake_process))
            stack.enter_context(caller_umask(0o077))
            args = types.SimpleNamespace(arch="aarch64", skip_build=True, build_only=False)
            env = {"MACOS_MAS_BUILD_NUMBER": "17", "MACOS_MAS_USES_NONEXEMPT_ENCRYPTION": "false",
                   "MACOS_MAS_BUILD_RECEIPT": str(receipt_path), "MACOS_MAS_BUILD_RECEIPT_SHA256": "a" * 64,
                   "MACOS_MAS_DIST": str(directory / "dist")}
            self.mas.package(root, self.mas.CONFIG["mixel-photo"], args, env)
        return events

    def test_real_package_gates_signed_and_expanded_roots(self):
        with self.subTest("private signer output fails before productbuild"):
            with self.assertRaisesRegex(self.mas.Failure, "not publicly readable"):
                self.package_fixture(self.root / "bad-stage", poison_stage=True)
            self.assertFalse(any(name == "productbuild" for name, _ in self.events))
            self.assertFalse((self.root / "bad-stage/dist").exists())
        with self.subTest("expanded app root fails before final copy"):
            with self.assertRaisesRegex(self.mas.Failure, "traversable: \\."):
                self.package_fixture(self.root / "bad-expanded", poison_expansion=True)
            self.assertTrue(any(name == "productbuild" for name, _ in self.events))
            self.assertFalse((self.root / "bad-expanded/dist").exists())
        with self.subTest("public signed and expanded app succeeds"):
            events = self.package_fixture(self.root / "public")
            self.assertEqual(sum(name == "productbuild" for name, _ in events), 1)
            self.assertEqual(len(list((self.root / "public/dist").glob("*.pkg"))), 1)
            codesign = [args for name, args in events if name == "codesign"]
            self.assertEqual(len(codesign), 2)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--helper", type=Path)
    options, remaining = parser.parse_known_args()
    if options.helper is not None:
        HELPER = options.helper.resolve()
    unittest.main(argv=[sys.argv[0], *remaining])
