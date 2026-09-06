#!/usr/bin/env python3
"""Regenerate the checked-in, path-free DMG Finder layout.

Optional authoring dependency: ds-store==1.3.3 (MIT, PyPI). It is not a runtime
or release-build dependency. No application is driven and no global Finder
preferences are changed; this writes one binary packaging asset.
"""

from pathlib import Path
from ds_store import DSStore

output = Path(__file__).resolve().parent / "packaging" / "installer-finder-layout.dsstore"
output.parent.mkdir(parents=True, exist_ok=True)
with DSStore.open(str(output), "w+") as store:
    store["."]["bwsp"] = {
        "ShowStatusBar": False, "ShowToolbar": False, "ShowSidebar": False,
        "ShowPathbar": False, "ShowTabView": False, "ContainerShowSidebar": False,
        "WindowBounds": "{{200, 160}, {640, 300}}", "PreviewPaneVisibility": False,
    }
    store["."]["icvp"] = {
        "viewOptionsVersion": 1, "backgroundType": 1,
        "backgroundColorRed": 1.0, "backgroundColorGreen": 1.0, "backgroundColorBlue": 1.0,
        "gridOffsetX": 0.0, "gridOffsetY": 0.0, "gridSpacing": 100.0,
        "arrangeBy": "none", "showIconPreview": True, "showItemInfo": False,
        "labelOnBottom": True, "textSize": 14.0, "iconSize": 96.0,
        "scrollPositionX": 0.0, "scrollPositionY": 0.0,
    }
    store["."]["vSrn"] = ("long", 1)
    store["."]["vstl"] = ("type", b"icnv")
    store["Install Lumi.pkg"]["Iloc"] = (180, 125)
    store["Licenses & Sources"]["Iloc"] = (460, 125)
print(output)
