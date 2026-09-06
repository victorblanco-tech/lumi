#!/usr/bin/env python3
"""Audit the actual installer archive; never install or modify user data."""

import importlib.util
import plistlib
import sys
import tempfile
from pathlib import Path
from xml.etree import ElementTree as ET

spec = importlib.util.spec_from_file_location("builder", Path(__file__).with_name("build-macos-installer.py"))
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)


def verify(package, source_app):
    if sys.flags.optimize:
        raise RuntimeError("Installer verification must run with assertions enabled")
    info, destination, _, receipt = builder.configuration(source_app)
    with tempfile.TemporaryDirectory(prefix="lumi-installer-verify-") as temporary:
        expanded = Path(temporary) / "expanded"
        builder.run("/usr/sbin/pkgutil", "--expand-full", package, expanded)
        distribution = ET.parse(expanded / "Distribution").getroot()
        domains = distribution.find("domains")
        assert domains.attrib == {"enable_anywhere": "false", "enable_currentUserHome": "false",
                                  "enable_localSystem": "true"}
        assert distribution.find("options").get("customize") == "never"
        assert distribution.find(".//must-close/app").get("id") == info["CFBundleIdentifier"]
        assert not distribution.findall("script"), "Unexpected installer JavaScript"
        component = expanded / "Lumi.pkg"
        package_info = ET.parse(component / "PackageInfo").getroot()
        assert package_info.get("install-location") == destination
        assert package_info.get("identifier") == receipt
        assert package_info.get("version") == str(info["CFBundleVersion"])
        assert package_info.get("relocatable") == "false"
        assert not package_info.findall("relocate/*"), "Relocatable app could overwrite a different channel"
        assert not (component / "Scripts").exists(), "Installer must not execute root scripts"
        assert package_info.find("scripts") is None
        payload = component / "Payload"
        assert sorted(p.name for p in payload.iterdir()) == [source_app.name], "Non-app payload"
        app = payload / source_app.name
        with (app / "Contents/Info.plist").open("rb") as file:
            assert plistlib.load(file) == info
        builder.run("/usr/bin/codesign", "--verify", "--deep", "--strict", app)
        bundle = package_info.find("bundle")
        assert bundle.get("path") == "./" + source_app.name
        assert bundle.get("id") == info["CFBundleIdentifier"]
        upgrade = package_info.find("upgrade-bundle/bundle")
        assert upgrade is not None and upgrade.get("id") == info["CFBundleIdentifier"]
        strict = package_info.find("strict-identifier/bundle")
        assert strict is not None and strict.get("id") == info["CFBundleIdentifier"]
        checked = package_info.find("bundle-version/bundle")
        assert checked is not None and checked.get("id") == info["CFBundleIdentifier"]
    print("Installer audit passed: fixed channel path, app-only upgrade, no install scripts, valid signature.")


if __name__ == "__main__":
    verify(Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve())
