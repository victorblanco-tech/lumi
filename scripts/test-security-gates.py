#!/usr/bin/env python3
"""Isolated audit orchestration tests: no network, Maven, or real databases."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


class SecurityGates(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="lumi-security-test-")
        self.addCleanup(self.temp.cleanup)
        root = Path(self.temp.name)
        scripts = root / "scripts"
        scripts.mkdir()
        self.script = scripts / "verify-security.sh"
        shutil.copyfile(Path(__file__).with_name("verify-security.sh"), self.script)
        self.log = root / "calls"
        self.env = dict(os.environ, CALLS=str(self.log), AUDIT_STATUS="0",
                        LICENSE_STATUS="0", OSV_RESPONSE='{"results":[{}]}')
        bin_dir = root / "bin"
        bin_dir.mkdir()
        self.env["PATH"] = f"{bin_dir}:{os.environ['PATH']}"
        self.env["LUMI_CARGO_AUDIT_EXECUTABLE"] = str(bin_dir / "audit")
        commands = {
            bin_dir / "audit": 'echo rust >> "$CALLS"; exit "$AUDIT_STATUS"',
            scripts / "verify-license-inventory.sh":
                'echo licenses >> "$CALLS"; exit "$LICENSE_STATUS"',
            bin_dir / "mvn": '''echo maven >> "$CALLS"
for arg in "$@"; do
  case "$arg" in -DoutputFile=*) echo 'example:library:jar:1.0:compile' > "${arg#*=}";; esac
done''',
            bin_dir / "curl": 'printf "%s" "$OSV_RESPONSE"',
        }
        for path, body in commands.items():
            path.write_text("#!/bin/bash\nset -eu\n" + body + "\n")
            path.chmod(0o755)

    def run_gate(self, mode):
        return subprocess.run(["bash", str(self.script), mode], env=self.env,
                              capture_output=True, text=True)

    def test_all_runs_remaining_gates_after_rust_failure(self):
        self.env["AUDIT_STATUS"] = "1"
        result = self.run_gate("all")
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual(self.log.read_text().splitlines(),
                         ["rust", "maven", "maven", "licenses"])

    def test_all_success(self):
        self.assertEqual(self.run_gate("all").returncode, 0)

    def test_independent_rust_gate(self):
        self.assertEqual(self.run_gate("rust").returncode, 0)
        self.assertEqual(self.log.read_text().strip(), "rust")

    def test_malformed_or_incomplete_osv_response_fails_closed(self):
        for response in ('{}', '{"results":[]}', '{"results":[{"error":"oops"}]}',
                         '{"results":[{"vulns":"invalid"}]}', 'not json'):
            with self.subTest(response=response):
                self.env["OSV_RESPONSE"] = response
                self.assertNotEqual(self.run_gate("maven").returncode, 0)

    def test_advisory_and_license_failure_are_not_hidden(self):
        self.env["OSV_RESPONSE"] = '{"results":[{"vulns":[{"id":"TEST-1"}]}]}'
        self.env["LICENSE_STATUS"] = "1"
        self.assertEqual(self.run_gate("all").returncode, 1)
        self.assertIn("licenses", self.log.read_text())

    def test_unknown_mode_rejected(self):
        self.assertEqual(self.run_gate("unknown").returncode, 2)
        self.assertFalse(self.log.exists())


if __name__ == "__main__":
    unittest.main()
