#!/usr/bin/env python3
"""Test runner identity checks without changing the host OS or Xcode."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


class CompatibilityEnvironment(unittest.TestCase):
    def check(self, expected="27", actual="27.0.1", xcode="27.2", sdk="27.2", arch="arm64"):
        with tempfile.TemporaryDirectory(prefix="lumi-platform-test-") as directory:
            root = Path(directory)
            for command, output in {
                "sw_vers": actual, "uname": arch,
                "xcodebuild": f"Xcode {xcode}\nBuild version TEST", "xcrun": sdk,
            }.items():
                executable = root / command
                executable.write_text(f"#!/bin/sh\nprintf '%s\\n' '{output}'\n")
                executable.chmod(0o755)
            result = subprocess.run(
                ["bash", str(Path(__file__).with_name("check-apple-compatibility-environment.sh"))],
                env=dict(os.environ, PATH=f"{root}:{os.environ['PATH']}",
                         EXPECTED_MACOS_MAJOR=expected, GITHUB_OUTPUT=str(root / "output")),
                capture_output=True, text=True,
            )
            output = (root / "output").read_text() if (root / "output").exists() else ""
            return result.returncode, output

    def test_accepts_both_supported_environments(self):
        self.assertEqual(self.check()[0], 0)
        self.assertEqual(self.check("26", "26.5.2", "26.6", "26.5")[0], 0)

    def test_rejects_wrong_os_architecture_or_toolchain(self):
        for kwargs in ({"actual": "26.5.2"}, {"xcode": "26.6"},
                       {"sdk": "26.5"}, {"arch": "x86_64"}, {"expected": "28"}):
            with self.subTest(kwargs=kwargs):
                status, output = self.check(**kwargs)
                self.assertNotEqual(status, 0)
                self.assertEqual(output, "")

    def test_cache_identity_changes_with_sdk_xcode_and_os(self):
        keys = {self.check()[1], self.check(sdk="27.3")[1],
                self.check(xcode="27.3")[1], self.check(actual="27.2.1")[1]}
        self.assertEqual(len(keys), 4)
        self.assertTrue(all(key.startswith("toolchain=") for key in keys))


if __name__ == "__main__":
    unittest.main()
