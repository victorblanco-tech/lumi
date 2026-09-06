#!/usr/bin/env python3
"""Portable regression tests for channel selection and installer input policy."""

import importlib.util
import plistlib
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location("builder", Path(__file__).with_name("build-macos-installer.py"))
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)


class InstallerConfigurationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="lumi-package-policy-")
        self.addCleanup(self.temporary.cleanup)

    def app(self, channel="release", version="0.6.3", name="Lumi.app", identifier=None, build="347"):
        app = Path(self.temporary.name) / name
        (app / "Contents").mkdir(parents=True, exist_ok=True)
        data = {"LumiReleaseChannel": channel, "LumiProductVersion": version,
                "CFBundleIdentifier": identifier or "co.victorblan.tech.lumi" + (
                    "." + channel if channel != "release" else ""), "CFBundleVersion": build}
        with (app / "Contents/Info.plist").open("wb") as target:
            plistlib.dump(data, target)
        return app

    def test_production_has_fixed_destination_and_stable_receipt(self):
        _, destination, display, receipt = builder.configuration(self.app())
        self.assertEqual(destination, "/Applications/Lumi")
        self.assertEqual(display, "Lumi")
        self.assertEqual(receipt, "co.victorblan.tech.lumi.installer")

    def test_dev_and_rc_keep_separate_paths_and_versioned_receipts(self):
        for channel, label in [("dev", "Dev"), ("rc", "RC")]:
            receipts = []
            for number in (1, 2):
                version = f"0.6.3-{channel}-{number}"
                app = self.app(channel, version, f"Lumi {label} {version}.app")
                _, destination, _, receipt = builder.configuration(app)
                self.assertEqual(destination, f"/Applications/Lumi/{label}")
                receipts.append(receipt)
            self.assertNotEqual(*receipts)

    def test_wrong_bundle_identity_rejected(self):
        with self.assertRaises(ValueError):
            builder.configuration(self.app(identifier="co.victorblan.tech.lumi.dev"))

    def test_wrong_name_rejected(self):
        with self.assertRaises(ValueError):
            builder.configuration(self.app(name="Lumi Dev.app"))

    def test_wrong_channel_version_rejected(self):
        for version in ("0.6.3-dev-1", "../../elsewhere", "0.6.3 release"):
            with self.subTest(version=version), self.assertRaises(ValueError):
                builder.configuration(self.app(version=version))

    def test_non_numeric_build_rejected(self):
        for build in ("0", "dev-1", "1.2", "-1"):
            with self.subTest(build=build), self.assertRaises(ValueError):
                builder.configuration(self.app(build=build))

    @unittest.skipUnless(sys.platform == "darwin", "Native packaging requires macOS")
    def test_real_archives_are_app_only_and_fixed_location_for_every_channel(self):
        verifier_spec = importlib.util.spec_from_file_location(
            "verifier", Path(__file__).with_name("verify-macos-installer.py"))
        verifier = importlib.util.module_from_spec(verifier_spec)
        verifier_spec.loader.exec_module(verifier)
        for channel, label in [("release", "Lumi"), ("dev", "Lumi Dev"), ("rc", "Lumi RC")]:
            with self.subTest(channel=channel):
                version = "0.6.3" + ("-" + channel + "-1" if channel != "release" else "")
                name = label + (" " + version if channel != "release" else "") + ".app"
                app = self.app(channel, version, name)
                executable = app / "Contents/MacOS/Lumi"
                executable.parent.mkdir()
                # Copy bytes/mode only, not Apple's restricted filesystem flags.
                shutil.copy("/usr/bin/true", executable)
                with (app / "Contents/Info.plist").open("rb") as source:
                    info = plistlib.load(source)
                info.update(CFBundleExecutable="Lumi", CFBundlePackageType="APPL",
                            CFBundleShortVersionString="0.6.3", LSMinimumSystemVersion="15.0")
                with (app / "Contents/Info.plist").open("wb") as target:
                    plistlib.dump(info, target)
                subprocess.run(["codesign", "--force", "--sign", "-", str(app)], check=True)
                package = Path(self.temporary.name) / (channel + ".pkg")
                builder.build_installer(app, package)
                verifier.verify(package, app)


if __name__ == "__main__":
    unittest.main()
