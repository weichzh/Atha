#!/usr/bin/env python3
"""Regenerate Atha launch and web icons from the approved SVG."""
from copy import deepcopy
import json
from pathlib import Path
import re
from shutil import copyfile
from subprocess import run
from tempfile import TemporaryDirectory
from xml.etree import ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
APP = ROOT / "reader/app"
TAURI = APP / "src-tauri"
ICONS = TAURI / "icons"
MASTER = ICONS / "app-icon.svg"
PUBLIC = APP / "public"
ANDROID = TAURI / "gen/android/app/src/main/res"
DENSITIES = {"mdpi": 48, "hdpi": 72, "xhdpi": 96, "xxhdpi": 144, "xxxhdpi": 192}
SVG = "http://www.w3.org/2000/svg"
ET.register_namespace("", SVG)


def svg_tag(name):
    return f"{{{SVG}}}{name}"


def write_svg(root, path):
    ET.indent(root, space="  ")
    path.write_bytes(ET.tostring(root, encoding="utf-8") + b"\n")


def render(source, path, size, background=None):
    options = ["--background-color", background] if background else []
    path.parent.mkdir(parents=True, exist_ok=True)
    run(
        ["rsvg-convert", "--width", str(size), "--height", str(size),
         *options, "--output", str(path), str(source)],
        check=True,
    )


def main():
    source = ET.parse(MASTER).getroot()
    color = source.find(f'./{svg_tag("rect")}[@id="background"]').attrib["fill"]
    glyph = [child for child in source if child.tag in (svg_tag("g"), svg_tag("path"))]

    with TemporaryDirectory(prefix="atha-icons-") as directory:
        temp = Path(directory)
        files: list[tuple[Path, Path]] = []
        adaptive = ET.Element(svg_tag("svg"), {"width": "1024", "height": "1024", "viewBox": source.get("viewBox")})
        # Fit the entire mark inside the 66 dp safe circle of the 108 dp layer.
        mark = ET.SubElement(adaptive, svg_tag("g"), {"transform": "translate(64 64) scale(.67) translate(-64 -64)"})
        mark.extend(deepcopy(glyph))
        write_svg(adaptive, temp / "foreground.svg")
        for element in mark.iter():
            if "fill" in element.attrib:
                element.set("fill", "#FFFFFF")
        write_svg(adaptive, temp / "monochrome.svg")
        manifest = {
            "default": str(MASTER),
            "bg_color": color,
            "android_fg": "foreground.svg",
            "android_monochrome": "monochrome.svg",
        }
        (temp / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
        output = temp / "tauri"
        run(["mise", "exec", "--", "pnpm", "--dir", "reader/app", "tauri", "icon",
             str(temp / "manifest.json"), "--output", str(output)], cwd=ROOT, check=True)
        for name in ("icon.png", "icon.ico", "icon.icns", "32x32.png", "128x128.png"):
            files.append((output / name, ICONS / name))
        # The Linux bundle needs the physical 256 px size at scale 1.
        files.append((output / "128x128@2x.png", ICONS / "256x256.png"))

        android_output = output / "android"
        for folder, names in {
            "values": ["ic_launcher_background.xml"],
            "mipmap-anydpi-v26": ["ic_launcher.xml", "ic_launcher_round.xml"],
        }.items():
            xml = android_output / folder / names[0]
            content = xml.read_text(encoding="utf-8")
            xml.write_text(content.rstrip() + "\n", encoding="utf-8")
            files.extend((xml, ANDROID / folder / name) for name in names)

        # The bitmap fallbacks use finished silhouettes. No mask is baked into either adaptive layer.
        rounded = deepcopy(source)
        rect = rounded.find(f'./{svg_tag("rect")}[@id="background"]')
        index = list(rounded).index(rect)
        rounded.remove(rect)
        rounded.insert(index, ET.Element(svg_tag("circle"), {"cx": "64", "cy": "64", "r": "64", "fill": color}))
        write_svg(rounded, temp / "round.svg")
        for density, size in DENSITIES.items():
            folder = f"mipmap-{density}"
            for name in ("ic_launcher_foreground.png", "ic_launcher_monochrome.png"):
                files.append((android_output / folder / name, ANDROID / folder / name))
            for name, artwork in (("ic_launcher.png", MASTER), ("ic_launcher_round.png", temp / "round.svg")):
                png = temp / folder / name
                render(artwork, png, size)
                files.append((png, ANDROID / folder / name))

        # Keep the doorway at 16 px; omit the binding aperture (the cover's second subpath).
        cover = source.find(f'./{svg_tag("path")}[@id="hardcover"]')
        paths = re.findall(r"[Mm][^Mm]+", cover.attrib["d"])
        if len(paths) != 2:
            raise ValueError("hardcover must have an outline subpath followed by the binding cutout")
        cover.set("d", paths[0].rstrip())
        write_svg(source, temp / "favicon.svg")
        render(temp / "favicon.svg", temp / "favicon.png", 32)
        render(MASTER, temp / "apple-touch-icon.png", 180, color)
        for name in ("favicon.svg", "favicon.png", "apple-touch-icon.png"):
            files.append((temp / name, PUBLIC / name))

        catalog = ICONS / "apple/Assets.xcassets/AppIcon.appiconset"
        render(MASTER, temp / "AppIcon.png", 1024, color)
        contents = {
            "images": [{"filename": "AppIcon.png", "idiom": "universal", "platform": "ios", "size": "1024x1024"}],
            "info": {"author": "xcode", "version": 1},
        }
        (temp / "Contents.json").write_text(json.dumps(contents, indent=2) + "\n", encoding="utf-8")
        for name in ("AppIcon.png", "Contents.json"):
            files.append((temp / name, catalog / name))

        # Publish only after both renderers have successfully produced the full set.
        for generated, destination in files:
            destination.parent.mkdir(parents=True, exist_ok=True)
            if not destination.exists() or destination.read_bytes() != generated.read_bytes():
                copyfile(generated, destination)


if __name__ == "__main__":
    main()
