#!/usr/bin/env python3

from __future__ import annotations

import re
import struct
import sys
import urllib.parse
import xml.etree.ElementTree as ET
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
MARKDOWN_FILES = sorted(ROOT.glob("*.md")) + sorted((ROOT / "docs").rglob("*.md"))
LINK_PATTERN = re.compile(r"!?\[[^\]]*\]\((<[^>]+>|[^)\s]+)(?:\s+['\"][^'\"]*['\"])?\)")
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def validate_markdown_links() -> list[str]:
    errors: list[str] = []
    for markdown_path in MARKDOWN_FILES:
        text = markdown_path.read_text(encoding="utf-8")
        for match in LINK_PATTERN.finditer(text):
            raw_target = match.group(1).strip("<>")
            target = urllib.parse.unquote(raw_target.split("#", 1)[0].split("?", 1)[0])
            if not target or target.startswith(("#", "http://", "https://", "mailto:")):
                continue
            if target.startswith("/"):
                continue
            resolved = (markdown_path.parent / target).resolve()
            try:
                resolved.relative_to(ROOT)
            except ValueError:
                errors.append(f"{markdown_path.relative_to(ROOT)}: link escapes repository: {raw_target}")
                continue
            if not resolved.exists():
                errors.append(f"{markdown_path.relative_to(ROOT)}: missing local target: {raw_target}")
    return errors


def validate_svg_files() -> list[str]:
    errors: list[str] = []
    for svg_path in sorted((ROOT / "docs").rglob("*.svg")):
        try:
            ET.parse(svg_path)
        except ET.ParseError as error:
            errors.append(f"{svg_path.relative_to(ROOT)}: invalid SVG XML: {error}")
    return errors


def validate_png_files() -> list[str]:
    errors: list[str] = []
    for png_path in sorted((ROOT / "docs").rglob("*.png")):
        data = png_path.read_bytes()[:24]
        if len(data) < 24 or data[:8] != PNG_SIGNATURE or data[12:16] != b"IHDR":
            errors.append(f"{png_path.relative_to(ROOT)}: invalid PNG header")
            continue
        width, height = struct.unpack(">II", data[16:24])
        if width == 0 or height == 0:
            errors.append(f"{png_path.relative_to(ROOT)}: invalid PNG dimensions {width}x{height}")
    return errors


def main() -> int:
    errors = validate_markdown_links() + validate_svg_files() + validate_png_files()
    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(
        f"Documentation verification passed ({len(MARKDOWN_FILES)} Markdown files, "
        f"{len(list((ROOT / 'docs').rglob('*.svg')))} SVGs, "
        f"{len(list((ROOT / 'docs').rglob('*.png')))} PNGs)."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
