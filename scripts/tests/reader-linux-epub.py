#!/usr/bin/env python3
"""Write public, original Linux acceptance fixtures; no private input."""

import argparse
import hashlib
import json
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZIP_STORED, ZipFile


def xhtml(body):
    return f'<html xmlns="http://www.w3.org/1999/xhtml"><head><title>合成验收</title></head><body>{body}</body></html>'


def write_epub(output, formulas):
    title = "Atha Linux Source Return" if not formulas else "Atha Linux Synthetic Mathematics"
    first = '<h1>卷首</h1><p>本书为 Linux 验收而原创。</p>'
    second = '<h1>原文章节</h1>' + '<p>原文锚点，原文锚点，原文锚点，原文锚点。</p>' * 48
    assets = {}
    if not formulas:
        first = '<p><img src="landscape.svg" width="360" height="180" alt="山景插图"/></p>' + first
        assets["landscape.svg"] = (
            '<svg xmlns="http://www.w3.org/2000/svg" width="360" height="180" viewBox="0 0 180 90">'
            '<rect width="180" height="90" fill="#efe8cc"/>'
            '<path d="M0 90L58 18l32 39 34-33 56 66Z" fill="#168575"/></svg>'
        )
    if formulas:
        second = '<h1>合成数学例题</h1>'
        for i in range(formulas):
            name = f"formula-{i}.svg"
            assets[name] = (
                '<svg xmlns="http://www.w3.org/2000/svg" width="180" height="48" viewBox="0 0 180 48">'
                f'<text x="4" y="30" font-size="19">x + {i} = y</text></svg>'
            )
            second += f'<p>例题 {i + 1}：<img src="{name}" class="math-display" alt="x + {i} = y" width="180" height="48"/></p>'
    manifest = ''.join(
        f'<item id="f{i}" href="{name}" media-type="image/svg+xml"/>' for i, name in enumerate(assets)
    )
    opf = f'''<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
<metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">{title}</dc:identifier>
<dc:title>{title}</dc:title><dc:language>zh</dc:language><meta property="dcterms:modified">2026-01-01T00:00:00Z</meta></metadata>
<manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
<item id="one" href="one.xhtml" media-type="application/xhtml+xml"/>
<item id="two" href="two.xhtml" media-type="application/xhtml+xml"/>{manifest}</manifest>
<spine><itemref idref="one"/><itemref idref="two"/></spine></package>'''
    nav = xhtml('<nav xmlns:epub="http://www.idpf.org/2007/ops" epub:type="toc"><ol>'
                '<li><a href="one.xhtml">卷首</a></li><li><a href="two.xhtml">原文章节</a></li></ol></nav>')
    container = ('<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0">'
                 '<rootfiles><rootfile full-path="book.opf" media-type="application/oebps-package+xml"/>'
                 '</rootfiles></container>')
    with ZipFile(output, "x", compression=ZIP_DEFLATED) as archive:
        archive.writestr("mimetype", "application/epub+zip", compress_type=ZIP_STORED)
        for name, contents in {
            "META-INF/container.xml": container, "book.opf": opf, "nav.xhtml": nav,
            "one.xhtml": xhtml(first), "two.xhtml": xhtml(second), **assets
        }.items():
            archive.writestr(name, contents)
    if formulas:
        output.with_suffix(".json").write_text(
            json.dumps({"source_sha256": hashlib.sha256(output.read_bytes()).hexdigest(), "entry": "two.xhtml"}) + "\n",
            encoding="utf-8",
        )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("epub", type=Path)
    parser.add_argument("--formulas", type=int, default=0)
    args = parser.parse_args()
    write_epub(args.epub, args.formulas)
