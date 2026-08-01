#!/usr/bin/env python3
"""Export one XHTML page or section and its direct resources from an EPUB."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
import posixpath
import re
import shutil
import stat
import sys
import time
import zipfile
from pathlib import Path, PurePosixPath
from urllib.parse import unquote, urlsplit
from xml.etree import ElementTree as ET

MAX_MEMBER_BYTES = 16 * 1024 * 1024
XHTML = "http://www.w3.org/1999/xhtml"
EPUB = "http://www.idpf.org/2007/ops"
XLINK = "http://www.w3.org/1999/xlink"
MARKER = ".atha-reader-sample.json"

ET.register_namespace("", XHTML)
ET.register_namespace("epub", EPUB)


class SampleError(ValueError):
    """An EPUB or requested sample violates the extraction boundary."""


def archive_name(value: str) -> str:
    if not value or "\0" in value or "\\" in value:
        raise SampleError(f"unsafe EPUB path: {value!r}")
    if value.startswith(("/", "//")) or re.match(r"^[A-Za-z]:", value):
        raise SampleError(f"absolute EPUB path: {value!r}")
    parts = PurePosixPath(value).parts
    if any(part in {"", ".", ".."} for part in parts):
        raise SampleError(f"traversing EPUB path: {value!r}")
    return "/".join(parts)


def inspect_archive(book: zipfile.ZipFile) -> dict[str, zipfile.ZipInfo]:
    members: dict[str, zipfile.ZipInfo] = {}
    for info in book.infolist():
        name = archive_name(info.filename.rstrip("/"))
        if name in members:
            raise SampleError(f"duplicate EPUB path: {name}")
        mode = info.external_attr >> 16
        if stat.S_ISLNK(mode):
            raise SampleError(f"symbolic link in EPUB: {name}")
        members[name] = info
    return members


def read_member(
    book: zipfile.ZipFile, members: dict[str, zipfile.ZipInfo], name: str
) -> bytes:
    name = archive_name(name)
    info = members.get(name)
    if info is None or info.is_dir():
        raise SampleError(f"missing EPUB file: {name}")
    if info.file_size > MAX_MEMBER_BYTES:
        raise SampleError(f"EPUB file is too large: {name}")
    return book.read(info)


def referenced_member(entry: str, value: str) -> str | None:
    parsed = urlsplit(value)
    if parsed.scheme or parsed.netloc:
        raise SampleError(f"external resource in sample: {value}")
    if not parsed.path:
        return None
    path = unquote(parsed.path)
    if "\\" in path:
        raise SampleError(f"unsafe resource path: {value}")
    resolved = posixpath.normpath(posixpath.join(posixpath.dirname(entry), path))
    return archive_name(resolved)


def local_name(element: ET.Element) -> str:
    return element.tag.rsplit("}", 1)[-1]


def select_section(tree: ET.ElementTree, section_id: str) -> None:
    root = tree.getroot()
    target = next((element for element in root.iter() if element.get("id") == section_id), None)
    if target is None:
        raise SampleError(f"section id not found: {section_id}")
    body = next((element for element in root.iter() if local_name(element) == "body"), None)
    if body is None:
        raise SampleError("XHTML body not found")
    for child in list(body):
        body.remove(child)
    body.text = "\n"
    selected = copy.deepcopy(target)
    selected.tail = "\n"
    body.append(selected)


def resource_members(tree: ET.ElementTree, entry: str) -> set[str]:
    resources: set[str] = set()
    for element in tree.getroot().iter():
        tag = local_name(element)
        reference = None
        if tag == "link" and "stylesheet" in element.get("rel", "").split():
            reference = element.get("href")
        elif tag in {"img", "source", "audio", "video", "track", "input"}:
            reference = element.get("src")
        elif tag == "object":
            reference = element.get("data")
        elif tag == "image":
            reference = element.get("href") or element.get(f"{{{XLINK}}}href")
        if reference:
            member = referenced_member(entry, reference)
            if member:
                resources.add(member)
    return resources


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def fixture_output(repo_root: Path, name: str) -> Path:
    if not name or Path(name).name != name or name in {".", ".."} or "\\" in name:
        raise SampleError("output must be one fixtures/local directory name")
    fixtures = (repo_root / "fixtures" / "local").resolve()
    output = (fixtures / name).resolve()
    if output.parent != fixtures:
        raise SampleError("output escapes fixtures/local")
    return output


def replace_output(staging: Path, output: Path) -> None:
    if output.exists():
        marker = output / MARKER
        if not marker.is_file():
            raise SampleError(f"refusing to replace unmanaged output: {output.name}")
        metadata = json.loads(marker.read_text(encoding="utf-8"))
        if metadata.get("generator") != "export_reader_sample.py":
            raise SampleError(f"refusing to replace foreign output: {output.name}")
        shutil.rmtree(output)
    for attempt in range(5):
        try:
            staging.replace(output)
            return
        except PermissionError:
            if attempt == 4:
                raise
            time.sleep(0.05 * (attempt + 1))


def export_sample(
    repo_root: Path, epub_path: Path, entry: str, section_id: str | None, output_name: str
) -> dict[str, object]:
    epub_path = epub_path.resolve(strict=True)
    if not epub_path.is_file():
        raise SampleError("EPUB is not a file")
    entry = archive_name(entry)
    output = fixture_output(repo_root, output_name)
    staging = output.with_name(f".{output.name}.staging-{os.getpid()}")
    if staging.exists():
        shutil.rmtree(staging)
    staging.mkdir(parents=True)
    source_hash = sha256(epub_path)
    try:
        with zipfile.ZipFile(epub_path) as book:
            members = inspect_archive(book)
            source = read_member(book, members, entry)
            try:
                tree = ET.ElementTree(ET.fromstring(source))
            except ET.ParseError as error:
                raise SampleError(f"invalid XHTML: {error}") from error
            if section_id:
                select_section(tree, section_id)
                page = ET.tostring(tree.getroot(), encoding="utf-8", xml_declaration=True)
            else:
                page = source
            files = {entry, *resource_members(tree, entry)}
            for name in sorted(files):
                data = page if name == entry else read_member(book, members, name)
                destination = staging.joinpath(*PurePosixPath(name).parts)
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(data)
        if sha256(epub_path) != source_hash:
            raise SampleError("source EPUB changed during extraction")
        metadata: dict[str, object] = {
            "generator": "export_reader_sample.py",
            "source_sha256": source_hash,
            "entry": entry,
            "section_id": section_id,
            "files": sorted(files),
        }
        (staging / MARKER).write_text(
            json.dumps(metadata, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
        )
        replace_output(staging, output)
        return metadata
    except Exception:
        shutil.rmtree(staging, ignore_errors=True)
        raise


def self_check(repo_root: Path) -> None:
    work = repo_root / ".tmp" / f"export-reader-sample-{os.getpid()}"
    source = work / "sample.epub"
    bad = work / "bad.epub"
    outputs = [".export-reader-section-check", ".export-reader-page-check"]
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)
    page = f"""<?xml version="1.0" encoding="utf-8"?>
<html xmlns="{XHTML}"><head><link rel="stylesheet" href="../styles/book.css" /></head>
<body><section id="drop"><h2>drop</h2><img src="../media/drop.png" /></section>
<section id="keep"><h2>keep</h2><img src="../media/keep.png" /></section></body></html>"""
    try:
        with zipfile.ZipFile(source, "w") as book:
            book.writestr("EPUB/text/ch.xhtml", page)
            book.writestr("EPUB/styles/book.css", "p { color: inherit; }")
            book.writestr("EPUB/media/drop.png", b"drop")
            book.writestr("EPUB/media/keep.png", b"keep")
        export_sample(repo_root, source, "EPUB/text/ch.xhtml", "keep", outputs[0])
        export_sample(repo_root, source, "EPUB/text/ch.xhtml", "keep", outputs[0])
        section_root = fixture_output(repo_root, outputs[0])
        section = (section_root / "EPUB/text/ch.xhtml").read_text(encoding="utf-8")
        assert "keep" in section and "drop" not in section
        assert (section_root / "EPUB/media/keep.png").is_file()
        assert not (section_root / "EPUB/media/drop.png").exists()
        export_sample(repo_root, source, "EPUB/text/ch.xhtml", None, outputs[1])
        whole = (fixture_output(repo_root, outputs[1]) / "EPUB/text/ch.xhtml").read_text(
            encoding="utf-8"
        )
        assert "keep" in whole and "drop" in whole
        with zipfile.ZipFile(bad, "w") as book:
            book.writestr("../escape", b"bad")
        try:
            export_sample(repo_root, bad, "EPUB/text/ch.xhtml", None, ".export-reader-bad")
        except SampleError:
            pass
        else:
            raise AssertionError("unsafe ZIP member was accepted")
    finally:
        shutil.rmtree(work, ignore_errors=True)
        for name in [*outputs, ".export-reader-bad"]:
            shutil.rmtree(fixture_output(repo_root, name), ignore_errors=True)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--epub", type=Path)
    parser.add_argument("--entry")
    parser.add_argument("--section-id")
    parser.add_argument("--output")
    parser.add_argument("--self-check", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repo_root = Path(__file__).resolve().parent.parent
    try:
        if args.self_check:
            self_check(repo_root)
            print("export_reader_sample: ok")
            return 0
        if not args.epub or not args.entry or not args.output:
            raise SampleError("--epub, --entry and --output are required")
        metadata = export_sample(repo_root, args.epub, args.entry, args.section_id, args.output)
        print(json.dumps(metadata, ensure_ascii=False))
        return 0
    except (OSError, SampleError, zipfile.BadZipFile, json.JSONDecodeError) as error:
        print(f"export_reader_sample: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
