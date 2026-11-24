#!/usr/bin/env python3
"""Ensure host isolation never changes the product deployment target or SDK."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

WRAPPER = Path(__file__).with_name("rustc-host-environment.sh")


class HostEnvironment(unittest.TestCase):
    def invoke(self, arguments, upstream=False):
        with tempfile.TemporaryDirectory(prefix="lumi-rust-host-") as directory:
            root = Path(directory)
            compiler = root / "fake compiler"
            compiler.write_text("#!/usr/bin/env python3\nimport json,os,sys\n"
                                "print(json.dumps({'args':sys.argv[1:], 'floor':os.getenv('MACOSX_DEPLOYMENT_TARGET'), "
                                "'sdk':os.getenv('SDKROOT'), 'upstream':os.getenv('SAW_UPSTREAM')}))\n")
            compiler.chmod(0o755)
            outer = root / "upstream"
            outer.write_text('#!/bin/bash\nexport SAW_UPSTREAM=yes\nexec "$@"\n')
            outer.chmod(0o755)
            result = subprocess.run(
                ["bash", str(WRAPPER), str(compiler), *arguments], check=True,
                env=dict(os.environ, MACOSX_DEPLOYMENT_TARGET="15.0", SDKROOT="selected SDK",
                         LUMI_UPSTREAM_RUSTC_WRAPPER=str(outer) if upstream else ""),
                capture_output=True, text=True,
            )
            return json.loads(result.stdout)

    def test_host_tools_drop_only_product_floor(self):
        for arguments in (["--crate-type", "proc-macro"], ["--crate-name", "build_script_build"]):
            result = self.invoke(arguments)
            self.assertIsNone(result["floor"])
            self.assertEqual(result["sdk"], "selected SDK")
            self.assertEqual(result["args"], arguments)

    def test_product_retains_floor_sdk_and_arguments(self):
        for target in (["--target", "aarch64-apple-darwin"], ["--target=aarch64-apple-darwin"]):
            arguments = [*target, "--crate-type", "bin", "path with spaces.rs"]
            result = self.invoke(arguments)
            self.assertEqual(result["floor"], "15.0")
            self.assertEqual(result["sdk"], "selected SDK")
            self.assertEqual(result["args"], arguments)

    def test_preserves_existing_compiler_wrapper(self):
        result = self.invoke(["--target=aarch64-apple-darwin"], upstream=True)
        self.assertEqual(result["upstream"], "yes")
        self.assertEqual(result["floor"], "15.0")


if __name__ == "__main__":
    unittest.main()
