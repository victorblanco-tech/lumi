#!/usr/bin/env python3
"""Wrap an already verified Lumi bundle in a fixed-location macOS Installer.

Build-time only: no executable install scripts, network access, user-data payload,
or Finder automation. pkgbuild owns the bundle replacement transaction.
"""

import argparse
import html
import plistlib
import re
import subprocess
import tempfile
from pathlib import Path
from xml.etree import ElementTree as ET


def run(*args):
    subprocess.run([str(arg) for arg in args], check=True)


def configuration(app):
    with (app / "Contents/Info.plist").open("rb") as source:
        info = plistlib.load(source)
    channel = info["LumiReleaseChannel"]
    suffix, destination, display = {
        "release": ("", "/Applications/Lumi", "Lumi"),
        "dev": (".dev", "/Applications/Lumi/Dev", "Lumi Dev"),
        "rc": (".rc", "/Applications/Lumi/RC", "Lumi RC"),
    }[channel]
    identifier = "co.victorblan.tech.lumi" + suffix
    version = info["LumiProductVersion"]
    build = str(info["CFBundleVersion"])
    pattern = r"\d+\.\d+\.\d+" + (rf"-{channel}-[1-9]\d*" if suffix else "")
    if not re.fullmatch(pattern, version) or not re.fullmatch(r"[1-9]\d*", build):
        raise ValueError("Invalid channel version or numeric build number")
    if info["CFBundleIdentifier"] != identifier:
        raise ValueError("Channel and bundle identifier do not agree")
    bundle = display + (" " + version if suffix else "") + ".app"
    if app.name != bundle:
        raise ValueError(f"Expected bundle name {bundle!r}, received {app.name!r}")
    # A versioned Dev/RC bundle gets its own receipt: installing another version
    # must not remove the older bundle as obsolete payload of the same receipt.
    receipt = identifier + ".installer" + ("." + version if suffix else "")
    return info, destination, display, receipt


def resource_page(title, body):
    return ("<!doctype html><html><head><meta charset='utf-8'><style>"
            "body{font:13px -apple-system,Helvetica,Arial,sans-serif;color:#222;"
            "background:#fff;margin:16px;line-height:1.35}"
            "h1{font-size:26px;margin:0 0 4px}h2{font-size:17px;margin-top:18px}"
            ".subtitle{color:#555;margin-top:0}.accent{color:#008bbd}"
            "code{font-size:12px}li{margin-bottom:8px}</style></head><body>"
            f"<h1>{html.escape(title)}</h1>{body}</body></html>")


def build_installer(app, output):
    info, destination, display, receipt = configuration(app)
    version, build = info["LumiProductVersion"], str(info["CFBundleVersion"])
    identifier = info["CFBundleIdentifier"]
    minimum_os = info.get("LSMinimumSystemVersion", "15.0")
    run("/usr/bin/codesign", "--verify", "--deep", "--strict", app)
    output.parent.mkdir(parents=True, exist_ok=True)
    if output.exists():
        raise FileExistsError(f"Refusing to overwrite {output}")
    with tempfile.TemporaryDirectory(prefix="lumi-installer-") as temporary:
        root = Path(temporary)
        payload, resources = root / "payload", root / "resources"
        payload.mkdir()
        resources.mkdir()
        run("/usr/bin/ditto", app, payload / app.name)
        component = [{"RootRelativeBundlePath": app.name,
                      "BundleIsRelocatable": False,
                      "BundleIsVersionChecked": True,
                      "BundleHasStrictIdentifier": True,
                      "BundleOverwriteAction": "upgrade"}]
        with (root / "components.plist").open("wb") as target:
            plistlib.dump(component, target)
        run("/usr/bin/pkgbuild", "--root", payload,
            "--component-plist", root / "components.plist",
            "--install-location", destination, "--identifier", receipt,
            "--version", build, "--ownership", "recommended", root / "Lumi.pkg")

        (resources / "welcome.html").write_text(resource_page(
            display, f"<p class='subtitle'>Version {html.escape(version)} · Apple Silicon</p>"
            "<p>Install or update Lumi. The destination folder is created automatically.</p>"
            "<ul><li>Your tracks, phrases, mappings and settings are kept.</li>"
            "<li>Other Lumi channels stay separate and are not changed.</li></ul>"
            f"<p>Installation folder<br><code>{html.escape(destination)}</code></p>"
            "<p>Finish your show and quit Lumi before continuing.<br>"
            "Administrator approval may be required.</p>"), encoding="utf-8")
        (resources / "conclusion.html").write_text(resource_page(
            "Lumi is installed", f"<p>Open <strong>{html.escape(display)}</strong> in "
            f"<code>{html.escape(destination)}</code>.</p>"
            "<p>You can now eject this disk image. Your existing library and "
            "configuration have not been reset.</p>"
            "<h2>First launch</h2><p>This public beta is not Apple-notarized. "
            "If macOS blocks Lumi, use <strong>System Settings → Privacy &amp; "
            "Security → Open Anyway</strong> for your verified download.</p>"), encoding="utf-8")

        distribution = ET.Element("installer-gui-script", {"minSpecVersion": "2"})
        ET.SubElement(distribution, "title").text = display + " " + version
        ET.SubElement(distribution, "options", {"customize": "never",
                      "require-scripts": "false", "hostArchitectures": "arm64"})
        ET.SubElement(distribution, "domains", {"enable_anywhere": "false",
                      "enable_currentUserHome": "false", "enable_localSystem": "true"})
        check = ET.SubElement(distribution, "volume-check")
        allowed = ET.SubElement(check, "allowed-os-versions")
        ET.SubElement(allowed, "os-version", {"min": minimum_os})
        for page in ("welcome", "conclusion"):
            ET.SubElement(distribution, page, {"file": page + ".html", "mime-type": "text/html"})
        outline = ET.SubElement(distribution, "choices-outline")
        ET.SubElement(outline, "line", {"choice": "lumi"})
        choice = ET.SubElement(distribution, "choice", {"id": "lumi", "visible": "false",
                               "title": display, "description": "Install " + display})
        ET.SubElement(choice, "pkg-ref", {"id": receipt})
        ET.SubElement(distribution, "pkg-ref", {"id": receipt, "version": build,
                      "onConclusion": "none"}).text = "Lumi.pkg"
        ref = ET.SubElement(distribution, "pkg-ref", {"id": receipt})
        close = ET.SubElement(ref, "must-close")
        ET.SubElement(close, "app", {"id": identifier})
        ET.ElementTree(distribution).write(root / "Distribution.xml", encoding="utf-8",
                                           xml_declaration=True)
        run("/usr/bin/productbuild", "--distribution", root / "Distribution.xml",
            "--package-path", root, "--resources", resources, output)
    print(f"Installer: {output}\nDestination: {destination}\nReceipt: {receipt}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("app", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    build_installer(args.app.resolve(), args.output.resolve())
