use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc, Barrier,
        atomic::{AtomicU64, Ordering},
    },
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use atha_backend::reader::{
    epub::{ImportError, READER_MANIFEST, import_epub},
    library::{LibraryError, LocalLibrary},
    resources::{BookRoot, ResourceError},
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x04, 0x00, 0x00, 0x00, 0xb5, 0x1c, 0x0c,
    0x02, 0x00, 0x00, 0x00, 0x0b, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x64, 0xf8, 0x0f, 0x00,
    0x01, 0x05, 0x01, 0x01, 0x27, 0x18, 0xe3, 0x66, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44,
    0xae, 0x42, 0x60, 0x82,
];

const JPEG_2X1: &[u8] = &[
    0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10, 0x4a, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00, 0x00, 0x01,
    0x00, 0x01, 0x00, 0x00, 0xff, 0xdb, 0x00, 0x43, 0x00, 0x03, 0x02, 0x02, 0x02, 0x02, 0x02, 0x03,
    0x02, 0x02, 0x02, 0x03, 0x03, 0x03, 0x03, 0x04, 0x06, 0x04, 0x04, 0x04, 0x04, 0x04, 0x08, 0x06,
    0x06, 0x05, 0x06, 0x09, 0x08, 0x0a, 0x0a, 0x09, 0x08, 0x09, 0x09, 0x0a, 0x0c, 0x0f, 0x0c, 0x0a,
    0x0b, 0x0e, 0x0b, 0x09, 0x09, 0x0d, 0x11, 0x0d, 0x0e, 0x0f, 0x10, 0x10, 0x11, 0x10, 0x0a, 0x0c,
    0x12, 0x13, 0x12, 0x10, 0x13, 0x0f, 0x10, 0x10, 0x10, 0xff, 0xc0, 0x00, 0x0b, 0x08, 0x00, 0x01,
    0x00, 0x02, 0x01, 0x01, 0x11, 0x00, 0xff, 0xc4, 0x00, 0x14, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x09, 0xff, 0xc4, 0x00, 0x14,
    0x10, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0xff, 0xda, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3f, 0x00, 0x54, 0xdf, 0xff, 0xd9,
];

const EXIF_ORIENTATION_6: &[u8] = &[
    0xff, 0xe1, 0x00, 0x22, b'E', b'x', b'i', b'f', 0x00, 0x00, b'I', b'I', 0x2a, 0x00, 0x08, 0x00,
    0x00, 0x00, 0x01, 0x00, 0x12, 0x01, 0x03, 0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
];

#[derive(Clone, Copy)]
enum EpubVariant {
    Valid,
    UnsizedImage,
    StyledUnsizedImage,
    OrientedImage,
    MalformedJpegImage,
    MalformedExifImage,
    LateExifPngImage,
    OversizedImage,
    Changed,
    InvalidMimetype,
    UnsafePath,
    Doctype,
    ContainerExternalDoctype,
    ExtraContainerRoot,
    ExtraPackageRoot,
    MultipleRootfiles,
    ExternalReference,
    Encryption,
    FontObfuscation,
    FontObfuscationContent,
    FontObfuscationUnknown,
    LargeOptionalFont,
    TruncatedNavigation,
    ExtensionlessXhtml,
    InvalidNavigationDoctype,
    DuplicateNavigationDoctype,
    ContainerDepthOverflow,
    PackageDepthOverflow,
    NavigationDepthOverflow,
    Epub3NcxFallback,
    Epub3NoNavigation,
    ManifestPathAlias,
    ManifestConflictingAlias,
    MissingSpineMember,
    MissingOptionalResources,
    Epub3UnsupportedCover,
    Epub2Ncx,
    Epub2NcxWithoutDoctype,
    Epub2NcxExternalReference,
    Epub2NcxExternalLabelMedia,
    Epub2NcxTrailingExternalLabelMedia,
    Epub2NcxNavMapExternalLabelMedia,
    Epub2NcxTraversal,
    Epub2MissingSpineToc,
    Epub2TruncatedNcx,
    Epub2UnknownNcxDoctype,
    Epub2NcxEntity,
    Epub2NcxOutOfOrder,
    Epub2NcxDepthLimit,
    Epub2NcxDepthOverflow,
    Epub2NcxIgnoredRootContent,
    Epub2NcxAlternateLabel,
    Epub2NcxMissingHead,
    Epub2NcxDuplicateHead,
    Epub2NcxMissingDocTitle,
    Epub2NcxDuplicateDocTitle,
    Epub2NcxRootOutOfOrder,
    Epub2NcxUnknownRoot,
    Epub2MissingCoverItem,
    Epub2UnsupportedCover,
    Epub2Utf16Xhtml,
    Epub2NcxDuplicateId,
    Epub2NcxDuplicateHref,
    Epub2NcxDuplicatePlayOrder,
    Epub2NcxEmptyLabel,
    Epub2NcxOversizeLabel,
    Epub2NcxWrongNamespace,
    Epub2NcxWrongVersion,
    Epub2NcxTocLimit,
    Epub2NcxTocOverflow,
}

#[test]
fn annotates_unsized_images_without_changing_the_source() {
    let root = TestRoot::new();
    let source = root.0.join("unsized-image.epub");
    write_epub(&source, EpubVariant::UnsizedImage);
    let original = fs::read(&source).expect("read source before import");

    let imported = import_epub(&source, root.0.join("cache")).expect("import unsized image");
    let section = fs::read_to_string(imported.root.join("OEBPS/text/one.xhtml"))
        .expect("read annotated section");

    assert!(section.contains(r#"src="../images/cover.png""#));
    assert!(section.contains(r#"width="1""#));
    assert!(section.contains(r#"height="1""#));
    assert!(section.contains(r#"data-atha-native-size="""#));
    assert_eq!(
        fs::read(source).expect("read source after import"),
        original
    );
}

#[test]
fn native_image_dimensions_preserve_authored_css_and_reject_unsafe_sizes() {
    let root = TestRoot::new();
    for (name, variant) in [
        ("styled.epub", EpubVariant::StyledUnsizedImage),
        ("oversized.epub", EpubVariant::OversizedImage),
    ] {
        let source = root.0.join(name);
        write_epub(&source, variant);
        let imported = import_epub(&source, root.0.join(format!("{name}-cache")))
            .expect("import without unsafe dimension hint");
        let section = fs::read_to_string(imported.root.join("OEBPS/text/one.xhtml"))
            .expect("read preserved section");
        if matches!(variant, EpubVariant::StyledUnsizedImage) {
            assert!(section.contains(r#"width="1""#), "{name}");
            assert!(section.contains(r#"height="1""#), "{name}");
            assert!(section.contains(r#"data-atha-native-size="""#), "{name}");
            assert!(section.contains("img { width: 50px; }"), "{name}");
            assert!(section.contains(r#"style="margin:auto""#), "{name}");
        } else {
            assert!(!section.contains(r#"width="9000""#), "{name}");
            assert!(!section.contains(r#"height="1""#), "{name}");
            assert!(!section.contains("data-atha-native-size"), "{name}");
        }
    }
}

#[test]
fn applies_exif_orientation_to_intrinsic_dimensions() {
    let root = TestRoot::new();
    let source = root.0.join("oriented.epub");
    write_epub(&source, EpubVariant::OrientedImage);

    let imported = import_epub(&source, root.0.join("cache")).expect("import oriented image");
    let section = fs::read_to_string(imported.root.join("OEBPS/text/one.xhtml"))
        .expect("read oriented section");

    assert!(section.contains(r#"width="1""#));
    assert!(section.contains(r#"height="2""#));
}

#[test]
fn annotates_jpeg_after_sof_when_scan_tail_is_missing() {
    let root = TestRoot::new();
    let source = root.0.join("malformed-jpeg.epub");
    write_epub(&source, EpubVariant::MalformedJpegImage);

    let imported = import_epub(&source, root.0.join("cache")).expect("import truncated JPEG scan");
    let section = fs::read_to_string(imported.root.join("OEBPS/text/one.xhtml"))
        .expect("read annotated section");

    assert!(section.contains(r#"width="2""#));
    assert!(section.contains(r#"height="1""#));
}

#[test]
fn skips_dimension_hint_when_exif_metadata_is_malformed() {
    let root = TestRoot::new();
    let source = root.0.join("malformed-exif.epub");
    write_epub(&source, EpubVariant::MalformedExifImage);

    let imported = import_epub(&source, root.0.join("cache")).expect("import malformed EXIF");
    let section = fs::read_to_string(imported.root.join("OEBPS/text/one.xhtml"))
        .expect("read preserved section");

    assert!(!section.contains(r#" width=""#));
    assert!(!section.contains(r#" height=""#));
}

#[test]
fn skips_dimension_hint_for_exif_after_png_image_data() {
    let root = TestRoot::new();
    let source = root.0.join("late-exif.epub");
    write_epub(&source, EpubVariant::LateExifPngImage);

    let imported = import_epub(&source, root.0.join("cache")).expect("import late PNG EXIF");
    let section = fs::read_to_string(imported.root.join("OEBPS/text/one.xhtml"))
        .expect("read preserved section");

    assert!(!section.contains(r#" width=""#));
    assert!(!section.contains(r#" height=""#));
}

#[test]
fn imports_extensionless_xhtml_with_html5_navigation_doctype() {
    let root = TestRoot::new();
    let source = root.0.join("extensionless.epub");
    write_epub(&source, EpubVariant::ExtensionlessXhtml);

    let imported = import_epub(&source, root.0.join("cache")).expect("import compatible epub");
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(imported.root.join(READER_MANIFEST)).expect("read reader manifest"),
    )
    .expect("parse reader manifest");
    assert_eq!(manifest["sections"][0]["href"], "OEBPS/text/one");

    let book = BookRoot::new(&imported.root).expect("open imported root");
    assert_eq!(
        book.read("/OEBPS/text/one")
            .expect("read extensionless xhtml")
            .content_type,
        "application/xhtml+xml; charset=utf-8"
    );
    fs::write(imported.root.join("OEBPS/text/undeclared"), b"not declared")
        .expect("write undeclared file");
    assert_eq!(
        book.read("/OEBPS/text/undeclared"),
        Err(ResourceError::UnsupportedMediaType)
    );
}

#[test]
fn imports_structurally_valid_epub_with_noncanonical_mimetype() {
    let root = TestRoot::new();
    let source = root.0.join("noncanonical-mimetype.epub");
    write_epub(&source, EpubVariant::InvalidMimetype);

    let imported = import_epub(&source, root.0.join("cache"))
        .expect("recognize EPUB from its bounded container and package");
    let book = BookRoot::new(&imported.root).expect("open imported root");
    book.read("/OEBPS/text/one.xhtml")
        .expect("read EPUB despite noncanonical mimetype");
}

#[test]
fn tolerates_safe_packaging_defects_without_guessing_content() {
    let root = TestRoot::new();

    for (name, variant) in [
        (
            "container-external-doctype.epub",
            EpubVariant::ContainerExternalDoctype,
        ),
        ("manifest-path-alias.epub", EpubVariant::ManifestPathAlias),
        ("missing-spine-member.epub", EpubVariant::MissingSpineMember),
    ] {
        let source = root.0.join(name);
        write_epub(&source, variant);
        let imported = import_epub(&source, root.0.join(format!("{name}-cache")))
            .expect("import recoverable packaging defect");
        let manifest: serde_json::Value = serde_json::from_slice(
            &fs::read(imported.root.join(READER_MANIFEST)).expect("read reader manifest"),
        )
        .expect("parse reader manifest");
        assert!(
            !manifest["sections"]
                .as_array()
                .expect("sections")
                .is_empty()
        );
    }

    let conflicting = root.0.join("manifest-conflicting-alias.epub");
    write_epub(&conflicting, EpubVariant::ManifestConflictingAlias);
    assert_eq!(
        import_epub(&conflicting, root.0.join("conflicting-cache")),
        Err(ImportError::InvalidXml)
    );
}

#[test]
fn omits_missing_optional_resources_without_rejecting_the_book() {
    let root = TestRoot::new();
    let source = root.0.join("missing-optional-resources.epub");
    write_epub(&source, EpubVariant::MissingOptionalResources);

    let imported = import_epub(&source, root.0.join("cache"))
        .expect("import body with missing optional resources");
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(imported.root.join(READER_MANIFEST)).expect("read reader manifest"),
    )
    .expect("parse reader manifest");
    assert_eq!(
        manifest["resources"],
        serde_json::json!(["OEBPS/styles/book.css"])
    );
    assert_eq!(imported.cover_path, None);
    assert!(!imported.root.join("OEBPS/images/missing.png").exists());
    let book = BookRoot::new(&imported.root).expect("open imported root");
    book.read("/OEBPS/text/one.xhtml")
        .expect("read preserved body");
    assert_eq!(
        book.read("/OEBPS/images/missing.png"),
        Err(ResourceError::NotFound)
    );
}

#[test]
fn ignores_only_allowlisted_obfuscated_fonts() {
    let root = TestRoot::new();
    for (name, variant) in [
        ("font-obfuscation.epub", EpubVariant::FontObfuscation),
        ("large-optional-font.epub", EpubVariant::LargeOptionalFont),
    ] {
        let source = root.0.join(name);
        write_epub(&source, variant);
        let imported = import_epub(&source, root.0.join(format!("{name}-cache")))
            .expect("read body with system-font fallback");
        assert!(!imported.root.join("OEBPS/fonts/book.ttf").exists());
        BookRoot::new(&imported.root)
            .expect("open imported root")
            .read("/OEBPS/text/one.xhtml")
            .expect("read body without embedded font");
    }

    for (name, variant) in [
        (
            "obfuscated-content.epub",
            EpubVariant::FontObfuscationContent,
        ),
        (
            "unknown-obfuscation.epub",
            EpubVariant::FontObfuscationUnknown,
        ),
    ] {
        let source = root.0.join(name);
        write_epub(&source, variant);
        assert_eq!(
            import_epub(&source, root.0.join(format!("{name}-cache"))),
            Err(ImportError::Encrypted),
            "{name}"
        );
    }
}

#[test]
fn imports_epub2_ncx_into_existing_reader_contract() {
    let root = TestRoot::new();
    let source = root.0.join("epub2-ncx.epub");
    write_epub(&source, EpubVariant::Epub2Ncx);

    let imported = import_epub(&source, root.0.join("cache")).expect("import EPUB2 with NCX");
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(imported.root.join(READER_MANIFEST)).expect("read reader manifest"),
    )
    .expect("parse reader manifest");

    assert_eq!(manifest["sections"].as_array().expect("sections").len(), 2);
    assert_eq!(manifest["sections"][0]["href"], "OEBPS/text/one.xhtml");
    assert_eq!(manifest["sections"][1]["href"], "OEBPS/text/two.xhtml");
    assert_eq!(
        manifest["toc"],
        serde_json::json!([
            { "label": "Part One", "href": "OEBPS/text/one.xhtml" },
            { "label": "Nested Two", "href": "OEBPS/text/two.xhtml#start" }
        ])
    );
    assert_eq!(imported.title.as_deref(), Some("Example EPUB2"));
    assert_eq!(imported.authors, ["Legacy Author"]);
    assert_eq!(
        imported.cover_path.as_deref(),
        Some("OEBPS/images/cover.png")
    );

    let book = BookRoot::new(&imported.root).expect("open imported root");
    let section = book
        .read("/OEBPS/text/one.xhtml")
        .expect("read XHTML 1.1 section");
    let xhtml = std::str::from_utf8(&section.bytes).expect("XHTML is UTF-8");
    assert!(xhtml.contains(
        r#"<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd">"#
    ));

    let no_doctype = root.0.join("epub2-ncx-no-doctype.epub");
    write_epub(&no_doctype, EpubVariant::Epub2NcxWithoutDoctype);
    import_epub(&no_doctype, root.0.join("no-doctype-cache"))
        .expect("import NCX without DTD or playOrder");
}

#[test]
fn imports_epub3_with_legacy_or_missing_navigation() {
    let root = TestRoot::new();

    let fallback = root.0.join("epub3-ncx-fallback.epub");
    write_epub(&fallback, EpubVariant::Epub3NcxFallback);
    let imported = import_epub(&fallback, root.0.join("fallback-cache"))
        .expect("import EPUB3 through its legacy NCX");
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(imported.root.join(READER_MANIFEST)).expect("read reader manifest"),
    )
    .expect("parse reader manifest");
    assert_eq!(manifest["sections"].as_array().expect("sections").len(), 2);
    assert_eq!(
        manifest["toc"],
        serde_json::json!([
            { "label": "Part One", "href": "OEBPS/text/one.xhtml" },
            { "label": "Nested Two", "href": "OEBPS/text/two.xhtml#start" }
        ])
    );
    let book = BookRoot::new(&imported.root).expect("open imported root");
    let second = book
        .read("/OEBPS/text/two.xhtml")
        .expect("read second section");
    assert!(
        std::str::from_utf8(&second.bytes)
            .expect("second section is UTF-8")
            .contains("fixture-body-two-7cb4")
    );

    let no_navigation = root.0.join("epub3-no-navigation.epub");
    write_epub(&no_navigation, EpubVariant::Epub3NoNavigation);
    let imported = import_epub(&no_navigation, root.0.join("no-navigation-cache"))
        .expect("import readable EPUB3 without navigation");
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(imported.root.join(READER_MANIFEST)).expect("read reader manifest"),
    )
    .expect("parse reader manifest");
    assert_eq!(manifest["sections"].as_array().expect("sections").len(), 2);
    assert_eq!(manifest["toc"], serde_json::json!([]));
}

#[test]
fn keeps_first_ncx_entry_when_targets_repeat() {
    let root = TestRoot::new();
    let source = root.0.join("epub2-ncx-duplicate-href.epub");
    write_epub(&source, EpubVariant::Epub2NcxDuplicateHref);

    let imported =
        import_epub(&source, root.0.join("cache")).expect("ignore duplicate NCX targets");
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(imported.root.join(READER_MANIFEST)).expect("read reader manifest"),
    )
    .expect("parse reader manifest");
    assert_eq!(
        manifest["toc"],
        serde_json::json!([
            { "label": "Part One", "href": "OEBPS/text/one.xhtml" }
        ])
    );
}

#[test]
fn imports_readable_spine_when_navigation_is_unusable() {
    let root = TestRoot::new();
    let cache = root.0.join("cache");
    for (name, variant) in [
        (
            "truncated-navigation.epub",
            EpubVariant::TruncatedNavigation,
        ),
        (
            "invalid-navigation-doctype.epub",
            EpubVariant::InvalidNavigationDoctype,
        ),
        (
            "duplicate-navigation-doctype.epub",
            EpubVariant::DuplicateNavigationDoctype,
        ),
        (
            "navigation-depth-overflow.epub",
            EpubVariant::NavigationDepthOverflow,
        ),
        (
            "epub2-ncx-external-label-media.epub",
            EpubVariant::Epub2NcxExternalLabelMedia,
        ),
        (
            "epub2-ncx-trailing-external-label-media.epub",
            EpubVariant::Epub2NcxTrailingExternalLabelMedia,
        ),
        (
            "epub2-ncx-nav-map-external-label-media.epub",
            EpubVariant::Epub2NcxNavMapExternalLabelMedia,
        ),
        (
            "epub2-missing-spine-toc.epub",
            EpubVariant::Epub2MissingSpineToc,
        ),
        ("epub2-truncated-ncx.epub", EpubVariant::Epub2TruncatedNcx),
        (
            "epub2-unknown-ncx-doctype.epub",
            EpubVariant::Epub2UnknownNcxDoctype,
        ),
        ("epub2-ncx-entity.epub", EpubVariant::Epub2NcxEntity),
        (
            "epub2-ncx-out-of-order.epub",
            EpubVariant::Epub2NcxOutOfOrder,
        ),
        (
            "epub2-ncx-depth-overflow.epub",
            EpubVariant::Epub2NcxDepthOverflow,
        ),
        (
            "epub2-ncx-missing-head.epub",
            EpubVariant::Epub2NcxMissingHead,
        ),
        (
            "epub2-ncx-duplicate-head.epub",
            EpubVariant::Epub2NcxDuplicateHead,
        ),
        (
            "epub2-ncx-missing-title.epub",
            EpubVariant::Epub2NcxMissingDocTitle,
        ),
        (
            "epub2-ncx-duplicate-title.epub",
            EpubVariant::Epub2NcxDuplicateDocTitle,
        ),
        (
            "epub2-ncx-root-out-of-order.epub",
            EpubVariant::Epub2NcxRootOutOfOrder,
        ),
        (
            "epub2-ncx-unknown-root.epub",
            EpubVariant::Epub2NcxUnknownRoot,
        ),
        (
            "epub2-ncx-duplicate-id.epub",
            EpubVariant::Epub2NcxDuplicateId,
        ),
        (
            "epub2-ncx-duplicate-play-order.epub",
            EpubVariant::Epub2NcxDuplicatePlayOrder,
        ),
        (
            "epub2-ncx-empty-label.epub",
            EpubVariant::Epub2NcxEmptyLabel,
        ),
        (
            "epub2-ncx-oversize-label.epub",
            EpubVariant::Epub2NcxOversizeLabel,
        ),
        (
            "epub2-ncx-wrong-namespace.epub",
            EpubVariant::Epub2NcxWrongNamespace,
        ),
        (
            "epub2-ncx-wrong-version.epub",
            EpubVariant::Epub2NcxWrongVersion,
        ),
        (
            "epub2-ncx-toc-overflow.epub",
            EpubVariant::Epub2NcxTocOverflow,
        ),
    ] {
        let source = root.0.join(name);
        write_epub(&source, variant);
        let imported = import_epub(&source, &cache).expect("ignore unusable optional navigation");
        let manifest: serde_json::Value = serde_json::from_slice(
            &fs::read(imported.root.join(READER_MANIFEST)).expect("read reader manifest"),
        )
        .expect("parse reader manifest");
        let sections = manifest["sections"].as_array().expect("sections");
        assert!(!sections.is_empty(), "{name}");
        assert_eq!(manifest["toc"], serde_json::json!([]), "{name}");
        let book = BookRoot::new(&imported.root).expect("open imported root");
        let href = sections[0]["href"].as_str().expect("section href");
        book.read(&format!("/{href}"))
            .expect("read section without navigation");
    }
}

#[test]
fn rejects_unsafe_epub2_ncx_references() {
    let root = TestRoot::new();
    let cache = root.0.join("cache");

    for (name, variant, expected) in [
        (
            "epub2-ncx-external.epub",
            EpubVariant::Epub2NcxExternalReference,
            ImportError::UnsafePath,
        ),
        (
            "epub2-ncx-traversal.epub",
            EpubVariant::Epub2NcxTraversal,
            ImportError::UnsafePath,
        ),
    ] {
        let source = root.0.join(name);
        write_epub(&source, variant);
        assert_eq!(import_epub(&source, &cache), Err(expected), "{name}");
    }
}

#[test]
fn enforces_one_xml_depth_limit_for_every_epub_document() {
    let root = TestRoot::new();
    let cache = root.0.join("cache");

    let at_limit = root.0.join("epub2-ncx-depth-limit.epub");
    write_epub(&at_limit, EpubVariant::Epub2NcxDepthLimit);
    import_epub(&at_limit, &cache).expect("import NCX at the XML depth limit");

    for (name, variant) in [
        (
            "container-depth-overflow.epub",
            EpubVariant::ContainerDepthOverflow,
        ),
        (
            "package-depth-overflow.epub",
            EpubVariant::PackageDepthOverflow,
        ),
    ] {
        let source = root.0.join(name);
        write_epub(&source, variant);
        assert_eq!(import_epub(&source, &cache), Err(ImportError::InvalidXml));
    }
}

#[test]
fn accepts_only_ordered_ncx_root_structure_and_first_usable_label() {
    let root = TestRoot::new();

    let ignored = root.0.join("epub2-ncx-ignored-root-content.epub");
    write_epub(&ignored, EpubVariant::Epub2NcxIgnoredRootContent);
    import_epub(&ignored, root.0.join("ignored-cache"))
        .expect("ignore bounded NCX docAuthor, pageList, and navList");

    let alternate = root.0.join("epub2-ncx-alternate-label.epub");
    write_epub(&alternate, EpubVariant::Epub2NcxAlternateLabel);
    let imported = import_epub(&alternate, root.0.join("alternate-cache"))
        .expect("use the first NCX navLabel with text");
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(imported.root.join(READER_MANIFEST)).expect("read reader manifest"),
    )
    .expect("parse reader manifest");
    assert_eq!(manifest["toc"][0]["label"], "Part One");
}

#[test]
fn ignores_invalid_legacy_cover_hints_but_rejects_unsupported_section_encoding() {
    let root = TestRoot::new();
    let cache = root.0.join("cache");

    for (name, variant) in [
        (
            "epub2-missing-cover-item.epub",
            EpubVariant::Epub2MissingCoverItem,
        ),
        (
            "epub2-unsupported-cover.epub",
            EpubVariant::Epub2UnsupportedCover,
        ),
        (
            "epub3-unsupported-cover.epub",
            EpubVariant::Epub3UnsupportedCover,
        ),
    ] {
        let source = root.0.join(name);
        write_epub(&source, variant);
        let imported = import_epub(&source, &cache).expect("ignore unusable cover hint");
        assert_eq!(imported.cover_path, None, "{name}");
    }

    let unsupported = root.0.join("epub2-utf16-xhtml.epub");
    write_epub(&unsupported, EpubVariant::Epub2Utf16Xhtml);
    assert_eq!(
        import_epub(&unsupported, &cache),
        Err(ImportError::UnsupportedEpub)
    );
}

#[test]
fn enforces_ncx_toc_item_limit() {
    let root = TestRoot::new();

    let at_limit = root.0.join("epub2-ncx-toc-limit.epub");
    write_epub(&at_limit, EpubVariant::Epub2NcxTocLimit);
    let imported = import_epub(&at_limit, root.0.join("limit-cache"))
        .expect("import NCX at the TOC item limit");
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(imported.root.join(READER_MANIFEST)).expect("read reader manifest"),
    )
    .expect("parse reader manifest");
    assert_eq!(manifest["toc"].as_array().expect("toc").len(), 2_000);
}

#[test]
fn imports_2000_sections_and_rejects_the_next() {
    let root = TestRoot::new();
    let at_limit = root.0.join("sections-at-limit.epub");
    write_many_section_epub(&at_limit, 2_000);
    let imported =
        import_epub(&at_limit, root.0.join("limit-cache")).expect("import EPUB at section limit");
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(imported.root.join(READER_MANIFEST)).expect("read reader manifest"),
    )
    .expect("parse reader manifest");
    assert_eq!(manifest["sections"].as_array().map(Vec::len), Some(2_000));
    let book = BookRoot::new(&imported.root).expect("open imported root");
    book.read("/OPS/text/1.xhtml").expect("read first section");
    book.read("/OPS/text/2000").expect("read last section");

    let overflow = root.0.join("sections-overflow.epub");
    write_many_section_epub(&overflow, 2_001);
    assert_eq!(
        import_epub(&overflow, root.0.join("overflow-cache")),
        Err(ImportError::TooManySections)
    );
}

#[test]
#[ignore = "writes the synthetic EPUB2 artifact used by target-platform gates"]
fn writes_epub2_gate_fixture() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../.tmp")
        .join("epub2-ncx-gate.epub");
    fs::create_dir_all(path.parent().expect("fixture parent")).expect("create fixture parent");
    write_epub(&path, EpubVariant::Epub2Ncx);
    assert!(path.metadata().is_ok_and(|metadata| metadata.len() > 0));
}

#[test]
#[ignore = "seeds an isolated Linux GUI formula benchmark from an explicit private EPUB"]
fn seeds_private_formula_gui_benchmark() {
    let root = PathBuf::from(
        std::env::var_os("ATHA_EPUB_GATE_LIBRARY_ROOT").expect("missing EPUB gate library root"),
    );
    let source =
        PathBuf::from(std::env::var_os("ATHA_EPUB_GATE_SOURCE").expect("missing EPUB gate source"));
    let temporary = fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp"))
        .expect("resolve .tmp root");
    let resolved = fs::canonicalize(&root).expect("resolve EPUB gate library root");
    assert!(
        root.is_absolute() && resolved.starts_with(temporary),
        "EPUB gate library must stay inside the repository .tmp directory"
    );
    let library = LocalLibrary::open(root).expect("open EPUB gate library");
    let source_bytes = source.metadata().expect("read EPUB gate metadata").len();
    let started = Instant::now();
    let book = library
        .stage_with_title_hint(&source, None)
        .expect("stage EPUB gate library");
    let stage_ms = started.elapsed().as_millis();
    assert!(!book.prepared);
    let started = Instant::now();
    let opened = library
        .open_book(&book.id)
        .expect("prepare seeded EPUB gate book through BookRoot");
    let first_open_ms = started.elapsed().as_millis();
    assert!(opened.book.prepared);
    let started = Instant::now();
    library
        .open_book(&book.id)
        .expect("reopen prepared EPUB gate book through BookRoot");
    let cached_open_us = started.elapsed().as_micros();
    eprintln!(
        "source_bytes={source_bytes} stage_ms={stage_ms} first_open_ms={first_open_ms} cached_open_us={cached_open_us}"
    );
}

struct TestRoot(PathBuf);
static TEST_ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

impl TestRoot {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.tmp")
            .join(format!(
                "atha-epub-import-{}-{nonce}-{}",
                std::process::id(),
                TEST_ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(&path).expect("create test root");
        Self(path)
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove test root");
    }
}

#[test]
fn imports_epub_and_rejects_unsafe_or_unsupported_sources() {
    let root = TestRoot::new();
    let source = root.0.join("book.epub");
    write_epub(&source, EpubVariant::Valid);
    let cache = root.0.join("cache");
    let imported = import_epub(&source, &cache).expect("import epub");
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(imported.root.join(READER_MANIFEST)).expect("read reader manifest"),
    )
    .expect("parse reader manifest");

    assert_eq!(manifest["contentVersion"], imported.content_version);
    assert_eq!(manifest["sections"].as_array().expect("sections").len(), 2);
    assert_eq!(manifest["sections"][0]["href"], "OEBPS/text/one.xhtml");
    assert_eq!(manifest["toc"].as_array().expect("toc").len(), 2);
    assert_eq!(manifest["toc"][1]["href"], "OEBPS/text/two.xhtml#start");
    assert_eq!(
        manifest["resources"],
        serde_json::json!(["OEBPS/images/cover.png", "OEBPS/styles/book.css"])
    );
    assert_eq!(imported.title.as_deref(), Some("Example Book"));
    assert_eq!(imported.authors, ["Example Author"]);
    assert_eq!(
        imported.cover_path.as_deref(),
        Some("OEBPS/images/cover.png")
    );
    assert!(imported.root.join("OEBPS/text/two.xhtml").is_file());

    let moved = root.0.join("moved.epub");
    fs::copy(&source, &moved).expect("copy epub");
    let repeated = import_epub(&moved, &cache).expect("reuse import");
    assert_eq!(repeated, imported);

    for (name, variant, expected) in [
        (
            "unsafe.epub",
            EpubVariant::UnsafePath,
            ImportError::UnsafePath,
        ),
        (
            "doctype.epub",
            EpubVariant::Doctype,
            ImportError::InvalidXml,
        ),
        (
            "extra-container-root.epub",
            EpubVariant::ExtraContainerRoot,
            ImportError::InvalidXml,
        ),
        (
            "extra-package-root.epub",
            EpubVariant::ExtraPackageRoot,
            ImportError::InvalidXml,
        ),
        (
            "external.epub",
            EpubVariant::ExternalReference,
            ImportError::UnsafePath,
        ),
        (
            "multiple-rootfiles.epub",
            EpubVariant::MultipleRootfiles,
            ImportError::UnsupportedEpub,
        ),
        (
            "encrypted.epub",
            EpubVariant::Encryption,
            ImportError::Encrypted,
        ),
    ] {
        let rejected = root.0.join(name);
        write_epub(&rejected, variant);
        assert_eq!(import_epub(&rejected, &cache), Err(expected));
    }

    let invalid_archive = root.0.join("invalid-archive.epub");
    fs::write(&invalid_archive, b"not a ZIP archive").expect("write invalid archive");
    assert_eq!(
        import_epub(&invalid_archive, &cache),
        Err(ImportError::InvalidArchive)
    );

    let oversized = root.0.join("oversized.epub");
    File::create(&oversized)
        .expect("create oversized epub")
        .set_len(512 * 1024 * 1024 + 1)
        .expect("size oversized epub");
    assert_eq!(
        import_epub(&oversized, &cache),
        Err(ImportError::SourceTooLarge)
    );
    assert_eq!(fs::read_dir(cache).expect("read cache").count(), 1);

    let changed_source = root.0.join("changed.epub");
    write_epub(&changed_source, EpubVariant::Changed);
    let changed = import_epub(&changed_source, root.0.join("cache")).expect("import changed epub");
    assert_ne!(changed.content_version, imported.content_version);
    assert_ne!(changed.root, imported.root);
}

#[test]
fn local_library_deduplicates_opens_and_removes_books_without_deleting_content() {
    let root = TestRoot::new();
    let source = root.0.join("book.epub");
    write_epub(&source, EpubVariant::Valid);
    let data = root.0.join("data");
    let library = LocalLibrary::open(&data).expect("open library");

    let imported = library.import(&source).expect("import into library");
    assert_eq!(imported.title, "Example Book");
    assert_eq!(imported.authors, ["Example Author"]);
    assert!(imported.has_cover);

    let moved = root.0.join("moved.epub");
    fs::copy(&source, &moved).expect("copy epub");
    assert_eq!(library.import(&moved).expect("repeat import"), imported);
    let listed = library.list().expect("list library");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0], imported);

    let opened = library.open_book(&imported.id).expect("open book");
    assert_eq!(opened.book, imported);
    assert!(opened.root.read(&format!("/{READER_MANIFEST}")).is_ok());
    assert_eq!(
        library
            .cover(&opened.book.id)
            .expect("read cover")
            .content_type,
        "image/png"
    );

    let reopened = LocalLibrary::open(&data).expect("reopen library");
    let listed = reopened.list().expect("list reopened");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0], opened.book);
    reopened.remove(&opened.book.id).expect("remove book");
    assert!(reopened.list().expect("list empty").is_empty());
    assert!(data.join("ImportedBooks").join(&opened.book.id).is_dir());
    assert_eq!(
        reopened.import(&moved).expect("restore book").id,
        opened.book.id
    );
    assert_eq!(
        reopened.open_book("not-a-content-hash").unwrap_err(),
        LibraryError::InvalidBookId
    );
    fs::write(
        data.join("Library")
            .join(format!("{}.json", opened.book.id)),
        b"{}",
    )
    .expect("corrupt library record");
    assert_eq!(reopened.list().unwrap_err(), LibraryError::CorruptRecord);
}

#[test]
fn opens_legacy_epub_caches_without_a_durable_source() {
    let root = TestRoot::new();
    let source = root.0.join("book.epub");
    write_epub(&source, EpubVariant::Valid);
    for version in [2, 3, 4] {
        let data = root.0.join(format!("data-v{version}"));
        let library = LocalLibrary::open(&data).expect("open library");
        let imported = library.import(&source).expect("create eager legacy record");
        let marker = data
            .join("ImportedBooks")
            .join(&imported.id)
            .join(".atha-epub-import");
        fs::write(
            &marker,
            format!("atha-epub-import-v{version}\n{}\n", imported.id),
        )
        .expect("write legacy marker");

        assert!(library.list().expect("list legacy cache")[0].prepared);
        library
            .open_book(&imported.id)
            .expect("open legacy cache without SourceBooks");
        assert!(
            data.join("SourceBooks")
                .read_dir()
                .expect("read sources")
                .next()
                .is_none()
        );
    }
}

#[test]
fn staged_library_import_prepares_on_first_open_and_reuses_the_cache() {
    let root = TestRoot::new();
    let source = root.0.join("book.epub");
    write_epub(&source, EpubVariant::Valid);
    let data = root.0.join("data");
    let library = LocalLibrary::open(&data).expect("open library");

    let staged = library
        .stage_with_title_hint(&source, Some("Picker title"))
        .expect("stage source");
    assert!(!staged.prepared);
    assert_eq!(staged.title, "Picker title");
    assert!(staged.authors.is_empty());
    assert!(!staged.has_cover);
    assert_eq!(
        fs::read_dir(data.join("SourceBooks"))
            .expect("read staged sources")
            .count(),
        1
    );
    assert_eq!(
        fs::read_dir(data.join("ImportedBooks"))
            .expect("read empty imports")
            .count(),
        0
    );

    let opened = library.open_book(&staged.id).expect("prepare first open");
    assert!(opened.book.prepared);
    assert_eq!(opened.book.title, "Example Book");
    assert_eq!(opened.book.authors, ["Example Author"]);
    assert!(opened.book.has_cover);
    assert!(opened.root.read(&format!("/{READER_MANIFEST}")).is_ok());

    let marker = data
        .join("ImportedBooks")
        .join(&staged.id)
        .join(".atha-epub-import");
    fs::write(&marker, format!("atha-epub-import-v4\n{}\n", staged.id))
        .expect("replace completion marker with legacy version");
    library
        .open_book(&staged.id)
        .expect("upgrade legacy cache from durable source");
    assert_eq!(
        fs::read_to_string(&marker).expect("read upgraded marker"),
        format!("atha-epub-import-v5\n{}\n", staged.id)
    );

    fs::write(&marker, format!("stale-import-version\n{}\n", staged.id))
        .expect("replace completion marker with a stale version");
    assert!(!library.list().expect("list incomplete book")[0].prepared);
    library
        .open_book(&staged.id)
        .expect("rebuild cache with a stale completion marker");

    for entry in fs::read_dir(data.join("SourceBooks")).expect("read prepared sources") {
        fs::remove_file(entry.expect("source entry").path()).expect("remove durable source probe");
    }
    let reopened = library
        .open_book(&staged.id)
        .expect("open prepared cache without source");
    assert_eq!(reopened.book, opened.book);
    assert_eq!(library.list().expect("list prepared book"), [opened.book]);

    fs::remove_file(
        data.join("ImportedBooks")
            .join(&staged.id)
            .join("OEBPS/styles/book.css"),
    )
    .expect("remove cached resource");
    assert_eq!(
        library.open_book(&staged.id).unwrap_err(),
        LibraryError::CorruptRecord
    );
}

#[test]
fn concurrent_first_opens_publish_one_complete_cache() {
    let root = TestRoot::new();
    let source = root.0.join("book.epub");
    write_epub(&source, EpubVariant::Valid);
    let library = LocalLibrary::open(root.0.join("data")).expect("open library");
    let staged = library
        .stage_with_title_hint(&source, None)
        .expect("stage source");
    let barrier = Arc::new(Barrier::new(4));

    std::thread::scope(|scope| {
        let handles = (0..4)
            .map(|_| {
                let library = library.clone();
                let id = staged.id.clone();
                let barrier = Arc::clone(&barrier);
                scope.spawn(move || {
                    barrier.wait();
                    library.open_book(&id)
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            let opened = handle
                .join()
                .expect("first-open worker")
                .expect("concurrent first open");
            assert!(opened.root.read(&format!("/{READER_MANIFEST}")).is_ok());
            assert!(opened.root.read("/OEBPS/styles/book.css").is_ok());
        }
    });
    let cache_entries = fs::read_dir(root.0.join("data/ImportedBooks"))
        .expect("read completed cache")
        .map(|entry| {
            entry
                .expect("cache entry")
                .file_name()
                .into_string()
                .expect("ASCII cache name")
        })
        .collect::<Vec<_>>();
    assert_eq!(cache_entries, [staged.id]);
}

#[test]
fn restaging_repairs_the_durable_source_and_legacy_record() {
    let root = TestRoot::new();
    let source = root.0.join("book.epub");
    write_epub(&source, EpubVariant::Valid);
    let data = root.0.join("data");
    let library = LocalLibrary::open(&data).expect("open library");
    let imported = library.import(&source).expect("create legacy eager record");
    fs::remove_dir_all(data.join("ImportedBooks").join(&imported.id))
        .expect("remove imported cache");

    let staged = library
        .stage_with_title_hint(&source, None)
        .expect("backfill durable source");
    assert!(!staged.prepared);
    let stored_source = fs::read_dir(data.join("SourceBooks"))
        .expect("read durable sources")
        .next()
        .expect("durable source")
        .expect("durable source entry")
        .path();
    fs::write(&stored_source, b"corrupt").expect("corrupt durable source");
    assert!(library.open_book(&imported.id).is_err());

    library
        .stage_with_title_hint(&source, None)
        .expect("repair durable source");
    library
        .open_book(&imported.id)
        .expect("prepare repaired source");

    let imported_root = data.join("ImportedBooks").join(&imported.id);
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(imported_root.join(READER_MANIFEST)).expect("read prepared manifest"),
    )
    .expect("parse prepared manifest");
    let section = manifest["sections"][0]["href"]
        .as_str()
        .expect("prepared section href");
    let resource = manifest["resources"][0]
        .as_str()
        .expect("prepared resource path");
    fs::remove_file(imported_root.join(section)).expect("remove cached section");

    let reopened = library
        .open_book(&imported.id)
        .expect("rebuild incomplete cache");
    assert!(reopened.root.read(&format!("/{section}")).is_ok());

    fs::remove_file(imported_root.join(resource)).expect("remove cached resource");
    let reopened = library
        .open_book(&imported.id)
        .expect("rebuild cache with missing resource");
    assert!(reopened.root.read(&format!("/{resource}")).is_ok());

    fs::write(imported_root.join(section), []).expect("empty cached section");
    let reopened = library
        .open_book(&imported.id)
        .expect("rebuild empty cached section");
    assert!(
        !reopened
            .root
            .read(&format!("/{section}"))
            .expect("read rebuilt section")
            .bytes
            .is_empty()
    );

    fs::write(
        data.join("Library").join(format!("{}.json", imported.id)),
        b"{}",
    )
    .expect("corrupt library record");
    let restaged = library
        .stage_with_title_hint(&source, None)
        .expect("reconstruct corrupt record");
    assert_eq!(restaged.id, imported.id);
    library
        .open_book(&imported.id)
        .expect("open reconstructed record");
}

fn oriented_jpeg() -> Vec<u8> {
    let mut bytes = JPEG_2X1.to_vec();
    bytes.splice(2..2, EXIF_ORIENTATION_6.iter().copied());
    bytes
}

fn jpeg_without_scan_marker() -> Vec<u8> {
    let scan = JPEG_2X1
        .windows(2)
        .position(|bytes| bytes == [0xff, 0xda])
        .expect("JPEG scan marker");
    JPEG_2X1[..scan].to_vec()
}

fn jpeg_with_malformed_exif() -> Vec<u8> {
    let mut bytes = JPEG_2X1.to_vec();
    bytes.splice(
        2..2,
        [
            0xff, 0xe1, 0x00, 0x0a, b'E', b'x', b'i', b'f', 0x00, 0x00, 0x00, 0x00,
        ],
    );
    bytes
}

fn oversized_png() -> Vec<u8> {
    let mut bytes = PNG_1X1.to_vec();
    bytes[16..20].copy_from_slice(&9_000_u32.to_be_bytes());
    bytes
}

fn png_with_late_exif() -> Vec<u8> {
    let mut bytes = PNG_1X1.to_vec();
    let iend = bytes
        .windows(4)
        .position(|chunk| chunk == b"IEND")
        .expect("PNG IEND")
        - 4;
    bytes.splice(
        iend..iend,
        [
            0x00, 0x00, 0x00, 0x04, b'e', b'X', b'I', b'f', 0x00, 0x00, 0x00, 0x00, 0x00, 0xd3,
            0x36, 0x0f, 0x2f,
        ],
    );
    bytes
}

fn write_epub(path: &Path, variant: EpubVariant) {
    let file = File::create(path).expect("create epub");
    let mut archive = ZipWriter::new(file);
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    archive.start_file("mimetype", stored).expect("mimetype");
    let mimetype = if matches!(variant, EpubVariant::InvalidMimetype) {
        b"application/zip".as_slice()
    } else {
        b"application/epub+zip".as_slice()
    };
    archive.write_all(mimetype).expect("write mimetype");
    if matches!(variant, EpubVariant::UnsafePath) {
        archive
            .start_file("../escape", stored)
            .expect("unsafe path");
        archive.write_all(b"escape").expect("write unsafe path");
        archive.finish().expect("finish unsafe epub");
        return;
    }
    if matches!(
        variant,
        EpubVariant::Encryption
            | EpubVariant::FontObfuscation
            | EpubVariant::FontObfuscationContent
            | EpubVariant::FontObfuscationUnknown
    ) {
        let (algorithm, target) = match variant {
            EpubVariant::FontObfuscation => {
                ("http://ns.adobe.com/pdf/enc#RC", "OEBPS/fonts/book.ttf")
            }
            EpubVariant::FontObfuscationContent => {
                ("http://www.idpf.org/2008/embedding", "OEBPS/text/one.xhtml")
            }
            EpubVariant::FontObfuscationUnknown => {
                ("https://example.com/encryption", "OEBPS/fonts/book.ttf")
            }
            _ => ("", ""),
        };
        let encryption = if matches!(variant, EpubVariant::Encryption) {
            "<encryption/>".to_owned()
        } else {
            format!(
                r#"<?xml version="1.0"?><encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container" xmlns:enc="http://www.w3.org/2001/04/xmlenc#"><enc:EncryptedData><enc:EncryptionMethod Algorithm="{algorithm}"/><enc:CipherData><enc:CipherReference URI="{target}"/></enc:CipherData></enc:EncryptedData></encryption>"#
            )
        };
        archive
            .start_file("META-INF/encryption.xml", stored)
            .expect("encryption marker");
        archive
            .write_all(encryption.as_bytes())
            .expect("write encryption marker");
    }
    let container = match variant {
        EpubVariant::Doctype => br#"<?xml version="1.0"?><!DOCTYPE container><container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0"><rootfiles><rootfile full-path="OEBPS/book.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#.to_vec(),
        EpubVariant::ContainerExternalDoctype => br#"<?xml version="1.0"?><!DOCTYPE container PUBLIC "-//EXAMPLE//DTD CONTAINER 1.0//EN" "https://example.com/container.dtd"><container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0"><rootfiles><rootfile full-path="OEBPS/book.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#.to_vec(),
        EpubVariant::ExtraContainerRoot => br#"<?xml version="1.0"?><extra/><container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0"><rootfiles><rootfile full-path="OEBPS/book.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#.to_vec(),
        EpubVariant::MultipleRootfiles => br#"<?xml version="1.0"?><container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0"><rootfiles><rootfile full-path="OEBPS/book.opf" media-type="application/oebps-package+xml"/><rootfile full-path="OEBPS/other.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#.to_vec(),
        EpubVariant::ContainerDepthOverflow => format!(
            r#"<?xml version="1.0"?><container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0"><rootfiles><rootfile full-path="OEBPS/book.opf" media-type="application/oebps-package+xml"/></rootfiles>{}</container>"#,
            nested_elements(255, true)
        )
        .into_bytes(),
        _ => br#"<?xml version="1.0"?><container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0"><rootfiles><rootfile full-path="OEBPS/book.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#.to_vec(),
    };
    let epub2 = matches!(
        variant,
        EpubVariant::Epub2Ncx
            | EpubVariant::Epub2NcxWithoutDoctype
            | EpubVariant::Epub2NcxExternalReference
            | EpubVariant::Epub2NcxExternalLabelMedia
            | EpubVariant::Epub2NcxTrailingExternalLabelMedia
            | EpubVariant::Epub2NcxNavMapExternalLabelMedia
            | EpubVariant::Epub2NcxTraversal
            | EpubVariant::Epub2MissingSpineToc
            | EpubVariant::Epub2TruncatedNcx
            | EpubVariant::Epub2UnknownNcxDoctype
            | EpubVariant::Epub2NcxEntity
            | EpubVariant::Epub2NcxOutOfOrder
            | EpubVariant::Epub2NcxDepthLimit
            | EpubVariant::Epub2NcxDepthOverflow
            | EpubVariant::Epub2NcxIgnoredRootContent
            | EpubVariant::Epub2NcxAlternateLabel
            | EpubVariant::Epub2NcxMissingHead
            | EpubVariant::Epub2NcxDuplicateHead
            | EpubVariant::Epub2NcxMissingDocTitle
            | EpubVariant::Epub2NcxDuplicateDocTitle
            | EpubVariant::Epub2NcxRootOutOfOrder
            | EpubVariant::Epub2NcxUnknownRoot
            | EpubVariant::Epub2MissingCoverItem
            | EpubVariant::Epub2UnsupportedCover
            | EpubVariant::Epub2Utf16Xhtml
            | EpubVariant::Epub2NcxDuplicateId
            | EpubVariant::Epub2NcxDuplicateHref
            | EpubVariant::Epub2NcxDuplicatePlayOrder
            | EpubVariant::Epub2NcxEmptyLabel
            | EpubVariant::Epub2NcxOversizeLabel
            | EpubVariant::Epub2NcxWrongNamespace
            | EpubVariant::Epub2NcxWrongVersion
            | EpubVariant::Epub2NcxTocLimit
            | EpubVariant::Epub2NcxTocOverflow
    );
    let package = if epub2 {
        epub2_package(variant)
    } else if matches!(variant, EpubVariant::PackageDepthOverflow) {
        format!(
            r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata/><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="one" href="text/one.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="one"/></spine>{}</package>"#,
            nested_elements(255, true)
        )
        .into_bytes()
    } else if matches!(variant, EpubVariant::ExternalReference) {
        br#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata/><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="one" href="https://example.com/one.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="one"/></spine></package>"#.to_vec()
    } else if matches!(variant, EpubVariant::ExtraPackageRoot) {
        br#"<?xml version="1.0"?><extra/><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata/><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="one" href="text/one.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="one"/></spine></package>"#.to_vec()
    } else if matches!(variant, EpubVariant::ExtensionlessXhtml) {
        br#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" xmlns:dc="http://purl.org/dc/elements/1.1/" version="3.0"><metadata><dc:title>Example Book</dc:title></metadata><manifest><item id="nav" href="nav" media-type="application/xhtml+xml" properties="nav"/><item id="one" href="text/one" media-type="application/xhtml+xml"/><item id="two" href="text/two" media-type="application/xhtml+xml"/><item id="css" href="styles/book.css" media-type="text/css"/></manifest><spine><itemref idref="one"/><itemref idref="two"/></spine></package>"#.to_vec()
    } else if matches!(variant, EpubVariant::Epub3NcxFallback) {
        br#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata/><manifest><item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/><item id="one" href="text/one.xhtml" media-type="application/xhtml+xml"/><item id="two" href="text/two.xhtml" media-type="application/xhtml+xml"/></manifest><spine toc="ncx"><itemref idref="one"/><itemref idref="two"/></spine></package>"#.to_vec()
    } else if matches!(variant, EpubVariant::Epub3NoNavigation) {
        br#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata/><manifest><item id="one" href="text/one.xhtml" media-type="application/xhtml+xml"/><item id="two" href="text/two.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="one"/><itemref idref="two"/></spine></package>"#.to_vec()
    } else if matches!(
        variant,
        EpubVariant::FontObfuscation
            | EpubVariant::FontObfuscationContent
            | EpubVariant::FontObfuscationUnknown
            | EpubVariant::LargeOptionalFont
    ) {
        br#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata/><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="one" href="text/one.xhtml" media-type="application/xhtml+xml"/><item id="two" href="text/two.xhtml" media-type="application/xhtml+xml"/><item id="font" href="fonts/book.ttf" media-type="application/x-font-ttf"/></manifest><spine><itemref idref="one"/><itemref idref="two"/></spine></package>"#.to_vec()
    } else if matches!(variant, EpubVariant::ManifestPathAlias) {
        br#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata/><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="one" href="text/one.xhtml" media-type="application/xhtml+xml"/><item id="two" href="text/two.xhtml" media-type="application/xhtml+xml"/><item id="css" href="styles/book.css" media-type="text/css"/><item id="css-alias" href="styles/book.css" media-type="text/css"/></manifest><spine><itemref idref="one"/><itemref idref="two"/></spine></package>"#.to_vec()
    } else if matches!(variant, EpubVariant::ManifestConflictingAlias) {
        br#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata/><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="one" href="text/one.xhtml" media-type="application/xhtml+xml"/><item id="two" href="text/two.xhtml" media-type="application/xhtml+xml"/><item id="css" href="styles/book.css" media-type="text/css"/><item id="css-alias" href="styles/book.css" media-type="image/png"/></manifest><spine><itemref idref="one"/><itemref idref="two"/></spine></package>"#.to_vec()
    } else if matches!(variant, EpubVariant::MissingSpineMember) {
        br#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata/><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="missing" href="text/missing.xhtml" media-type="application/xhtml+xml"/><item id="one" href="text/one.xhtml" media-type="application/xhtml+xml"/><item id="two" href="text/two.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="missing"/><itemref idref="one"/><itemref idref="two"/></spine></package>"#.to_vec()
    } else if matches!(variant, EpubVariant::MissingOptionalResources) {
        br#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata/><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="one" href="text/one.xhtml" media-type="application/xhtml+xml"/><item id="two" href="text/two.xhtml" media-type="application/xhtml+xml"/><item id="css" href="styles/book.css" media-type="text/css"/><item id="missing" href="images/missing.png" media-type="image/png"/><item id="cover" href="images/cover-missing.png" media-type="image/png" properties="cover-image"/></manifest><spine><itemref idref="one"/><itemref idref="two"/></spine></package>"#.to_vec()
    } else if matches!(variant, EpubVariant::Epub3UnsupportedCover) {
        br#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata/><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="one" href="text/one.xhtml" media-type="application/xhtml+xml"/><item id="two" href="text/two.xhtml" media-type="application/xhtml+xml"/><item id="css-cover" href="styles/book.css" media-type="text/css" properties="cover-image"/></manifest><spine><itemref idref="one"/><itemref idref="two"/></spine></package>"#.to_vec()
    } else {
        br#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" xmlns:dc="http://purl.org/dc/elements/1.1/" version="3.0"><metadata><dc:title> Example   Book </dc:title><dc:creator>Example Author</dc:creator></metadata><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="one" href="text/one.xhtml" media-type="application/xhtml+xml"/><item id="two" href="text/two.xhtml" media-type="application/xhtml+xml"></item><item id="css" href="styles/book.css" media-type="text/css"/><item id="cover" href="images/cover.png" media-type="image/png" properties="cover-image"/></manifest><spine><itemref idref="one"/><itemref idref="two"></itemref></spine></package>"#.to_vec()
    };
    let jpeg_cover = matches!(
        variant,
        EpubVariant::OrientedImage
            | EpubVariant::MalformedJpegImage
            | EpubVariant::MalformedExifImage
    );
    let package = if jpeg_cover {
        String::from_utf8(package)
            .expect("UTF-8 package")
            .replace("images/cover.png", "images/cover.jpg")
            .replace("image/png", "image/jpeg")
            .into_bytes()
    } else {
        package
    };
    let navigation = if matches!(variant, EpubVariant::Epub2NcxWithoutDoctype) {
        br#"<?xml version="1.0"?><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><head><meta name="dtb:uid" content="urn:uuid:00000000-0000-0000-0000-000000000002"/></head><docTitle><text>Example EPUB2</text></docTitle><navMap><navPoint id="one"><navLabel><text>One</text></navLabel><content src="text/one.xhtml"/></navPoint></navMap></ncx>"#.to_vec()
    } else if matches!(variant, EpubVariant::Epub2NcxExternalReference) {
        br#"<?xml version="1.0"?><!DOCTYPE ncx PUBLIC "-//NISO//DTD ncx 2005-1//EN" "http://www.daisy.org/z3986/2005/ncx-2005-1.dtd"><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><head><meta name="dtb:uid" content="urn:uuid:00000000-0000-0000-0000-000000000002"/><meta name="dtb:depth" content="1"/><meta name="dtb:totalPageCount" content="0"/><meta name="dtb:maxPageNumber" content="0"/></head><docTitle><text>Example EPUB2</text></docTitle><navMap><navPoint id="one" playOrder="1"><navLabel><text>One</text></navLabel><content src="https://example.com/one.xhtml"/></navPoint></navMap></ncx>"#.to_vec()
    } else if matches!(variant, EpubVariant::Epub2NcxExternalLabelMedia) {
        br#"<?xml version="1.0"?><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><head><meta name="dtb:uid" content="fixture"/></head><docTitle><text>Fixture</text></docTitle><navMap><navPoint id="one"><navLabel><audio src="https://example.com/label.mp3"/></navLabel><navLabel><text>One</text></navLabel><content src="text/one.xhtml"/></navPoint></navMap></ncx>"#.to_vec()
    } else if matches!(variant, EpubVariant::Epub2NcxTrailingExternalLabelMedia) {
        br#"<?xml version="1.0"?><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><head><meta name="dtb:uid" content="fixture"/></head><docTitle><text>Fixture</text></docTitle><navMap><navPoint id="one"><navLabel><text>One</text></navLabel><navLabel><img src="https://example.com/label.png"/></navLabel><content src="text/one.xhtml"/></navPoint></navMap></ncx>"#.to_vec()
    } else if matches!(variant, EpubVariant::Epub2NcxNavMapExternalLabelMedia) {
        br#"<?xml version="1.0"?><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><head><meta name="dtb:uid" content="fixture"/></head><docTitle><text>Fixture</text></docTitle><navMap><navLabel><audio src="https://example.com/map.mp3"/></navLabel><navPoint id="one"><navLabel><text>One</text></navLabel><content src="text/one.xhtml"/></navPoint></navMap></ncx>"#.to_vec()
    } else if matches!(variant, EpubVariant::Epub2NcxTraversal) {
        br#"<?xml version="1.0"?><!DOCTYPE ncx PUBLIC "-//NISO//DTD ncx 2005-1//EN" "http://www.daisy.org/z3986/2005/ncx-2005-1.dtd"><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><head><meta name="dtb:uid" content="urn:uuid:00000000-0000-0000-0000-000000000002"/><meta name="dtb:depth" content="1"/><meta name="dtb:totalPageCount" content="0"/><meta name="dtb:maxPageNumber" content="0"/></head><docTitle><text>Example EPUB2</text></docTitle><navMap><navPoint id="one" playOrder="1"><navLabel><text>One</text></navLabel><content src="../../escape.xhtml"/></navPoint></navMap></ncx>"#.to_vec()
    } else if matches!(variant, EpubVariant::Epub2TruncatedNcx) {
        br#"<?xml version="1.0"?><!DOCTYPE ncx PUBLIC "-//NISO//DTD ncx 2005-1//EN" "http://www.daisy.org/z3986/2005/ncx-2005-1.dtd"><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><head><meta name="dtb:uid" content="urn:uuid:00000000-0000-0000-0000-000000000002"/><meta name="dtb:depth" content="1"/><meta name="dtb:totalPageCount" content="0"/><meta name="dtb:maxPageNumber" content="0"/></head><docTitle><text>Example EPUB2</text></docTitle><navMap><navPoint id="one" playOrder="1"><navLabel><text>One"#.to_vec()
    } else if matches!(variant, EpubVariant::Epub2UnknownNcxDoctype) {
        br#"<?xml version="1.0"?><!DOCTYPE ncx PUBLIC "-//UNKNOWN//DTD NCX 1.0//EN" "https://example.com/unknown.dtd"><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><head><meta name="dtb:uid" content="urn:uuid:00000000-0000-0000-0000-000000000002"/><meta name="dtb:depth" content="1"/><meta name="dtb:totalPageCount" content="0"/><meta name="dtb:maxPageNumber" content="0"/></head><docTitle><text>Example EPUB2</text></docTitle><navMap><navPoint id="one" playOrder="1"><navLabel><text>One</text></navLabel><content src="text/one.xhtml"/></navPoint></navMap></ncx>"#.to_vec()
    } else if matches!(variant, EpubVariant::Epub2NcxEntity) {
        br#"<?xml version="1.0"?><!DOCTYPE ncx PUBLIC "-//NISO//DTD ncx 2005-1//EN" "http://www.daisy.org/z3986/2005/ncx-2005-1.dtd" [<!ENTITY injected "Injected label">]><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><head><meta name="dtb:uid" content="urn:uuid:00000000-0000-0000-0000-000000000002"/><meta name="dtb:depth" content="1"/><meta name="dtb:totalPageCount" content="0"/><meta name="dtb:maxPageNumber" content="0"/></head><docTitle><text>Example EPUB2</text></docTitle><navMap><navPoint id="one" playOrder="1"><navLabel><text>&injected;</text></navLabel><content src="text/one.xhtml"/></navPoint></navMap></ncx>"#.to_vec()
    } else if matches!(variant, EpubVariant::Epub2NcxOutOfOrder) {
        br#"<?xml version="1.0"?><!DOCTYPE ncx PUBLIC "-//NISO//DTD ncx 2005-1//EN" "http://www.daisy.org/z3986/2005/ncx-2005-1.dtd"><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><navMap><navPoint id="one" playOrder="1"><content src="text/one.xhtml"/><navLabel><text>One</text></navLabel></navPoint></navMap></ncx>"#.to_vec()
    } else if matches!(variant, EpubVariant::Epub2NcxDepthLimit) {
        format!(
            r#"<?xml version="1.0"?><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><head><meta name="dtb:uid" content="fixture"/></head><docTitle><text>Fixture</text></docTitle><docAuthor>{}</docAuthor><navMap><navPoint id="one"><navLabel><text>One</text></navLabel><content src="text/one.xhtml"/></navPoint></navMap></ncx>"#,
            nested_elements(253, true)
        )
        .into_bytes()
    } else if matches!(variant, EpubVariant::Epub2NcxDepthOverflow) {
        format!(
            r#"<?xml version="1.0"?><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><head><meta name="dtb:uid" content="fixture"/></head><docTitle><text>Fixture</text></docTitle><docAuthor>{}</docAuthor><navMap><navPoint id="one"><navLabel><text>One</text></navLabel><content src="text/one.xhtml"/></navPoint></navMap></ncx>"#,
            nested_elements(254, true)
        )
        .into_bytes()
    } else if matches!(
        variant,
        EpubVariant::Epub2NcxIgnoredRootContent
            | EpubVariant::Epub2NcxAlternateLabel
            | EpubVariant::Epub2NcxMissingHead
            | EpubVariant::Epub2NcxDuplicateHead
            | EpubVariant::Epub2NcxMissingDocTitle
            | EpubVariant::Epub2NcxDuplicateDocTitle
            | EpubVariant::Epub2NcxRootOutOfOrder
            | EpubVariant::Epub2NcxUnknownRoot
            | EpubVariant::Epub2NcxDuplicateId
            | EpubVariant::Epub2NcxDuplicateHref
            | EpubVariant::Epub2NcxDuplicatePlayOrder
            | EpubVariant::Epub2NcxEmptyLabel
            | EpubVariant::Epub2NcxOversizeLabel
            | EpubVariant::Epub2NcxWrongNamespace
            | EpubVariant::Epub2NcxWrongVersion
            | EpubVariant::Epub2NcxTocLimit
            | EpubVariant::Epub2NcxTocOverflow
    ) {
        epub2_navigation_case(variant)
    } else if epub2 || matches!(variant, EpubVariant::Epub3NcxFallback) {
        br#"<?xml version="1.0"?><!DOCTYPE ncx PUBLIC "-//NISO//DTD ncx 2005-1//EN" "http://www.daisy.org/z3986/2005/ncx-2005-1.dtd"><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><head><meta name="dtb:uid" content="urn:uuid:00000000-0000-0000-0000-000000000002"/><meta name="dtb:depth" content="2"/><meta name="dtb:totalPageCount" content="0"/><meta name="dtb:maxPageNumber" content="0"/></head><docTitle><text>Example EPUB2</text></docTitle><navMap><navInfo><text>Navigation information</text></navInfo><navLabel><text>Table of Contents</text></navLabel><navPoint id="one" playOrder="1"><navLabel xml:lang="en"><text> Part One </text></navLabel><navLabel xml:lang="zh"><text>Ignored alternate</text></navLabel><content src="text/one.xhtml"/><navPoint id="two" playOrder="2"><navLabel><text>Nested Two</text></navLabel><content src="text/two.xhtml#start"/></navPoint></navPoint></navMap></ncx>"#.to_vec()
    } else if matches!(variant, EpubVariant::NavigationDepthOverflow) {
        format!(
            r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="text/one.xhtml">One</a></li></ol></nav>{}</body></html>"#,
            nested_elements(254, true)
        )
        .into_bytes()
    } else if matches!(variant, EpubVariant::TruncatedNavigation) {
        br#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="text/one.xhtml">One"#.to_vec()
    } else if matches!(variant, EpubVariant::InvalidNavigationDoctype) {
        br#"<?xml version="1.0"?><!DOCTYPE svg><html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="text/one.xhtml">One</a></li></ol></nav></body></html>"#.to_vec()
    } else if matches!(variant, EpubVariant::DuplicateNavigationDoctype) {
        br#"<?xml version="1.0"?><!DOCTYPE html><!DOCTYPE html><html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="text/one.xhtml">One</a></li></ol></nav></body></html>"#.to_vec()
    } else if matches!(variant, EpubVariant::ExtensionlessXhtml) {
        br#"<?xml version="1.0"?><!DOCTYPE html><html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="text/one">One</a></li><li><a href="text/two#start">Two</a></li></ol></nav></body></html>"#.to_vec()
    } else {
        br#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="text/one.xhtml">One</a></li><li><a href="text/two.xhtml#start"><span>Two</span></a></li></ol></nav></body></html>"#.to_vec()
    };
    let extensionless = matches!(variant, EpubVariant::ExtensionlessXhtml);
    let nav_path = if epub2 || matches!(variant, EpubVariant::Epub3NcxFallback) {
        "OEBPS/toc.ncx"
    } else if extensionless {
        "OEBPS/nav"
    } else {
        "OEBPS/nav.xhtml"
    };
    let one_path = if extensionless {
        "OEBPS/text/one"
    } else {
        "OEBPS/text/one.xhtml"
    };
    let two_path = if extensionless {
        "OEBPS/text/two"
    } else {
        "OEBPS/text/two.xhtml"
    };
    let one = if matches!(variant, EpubVariant::StyledUnsizedImage) {
        br#"<html xmlns="http://www.w3.org/1999/xhtml"><head><style>img { width: 50px; }</style></head><body><img src="../images/cover.png" alt="Cover" style="margin:auto"/>One</body></html>"#.to_vec()
    } else if matches!(
        variant,
        EpubVariant::UnsizedImage
            | EpubVariant::OrientedImage
            | EpubVariant::MalformedJpegImage
            | EpubVariant::MalformedExifImage
            | EpubVariant::LateExifPngImage
            | EpubVariant::OversizedImage
    ) {
        br#"<html xmlns="http://www.w3.org/1999/xhtml"><body><img src="../images/cover.png" alt="Cover"/>One</body></html>"#.to_vec()
    } else if matches!(variant, EpubVariant::MissingOptionalResources) {
        br#"<html xmlns="http://www.w3.org/1999/xhtml"><head><link rel="stylesheet" href="../styles/book.css"/></head><body><img src="../images/missing.png" alt="Missing illustration"/>Body</body></html>"#.to_vec()
    } else if matches!(variant, EpubVariant::Epub2Utf16Xhtml) {
        let mut bytes = vec![0xff, 0xfe];
        for unit in r#"<?xml version="1.0" encoding="UTF-16"?><html xmlns="http://www.w3.org/1999/xhtml"><body><p>unsupported</p></body></html>"#.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        bytes
    } else if epub2 || matches!(variant, EpubVariant::Epub3NcxFallback) {
        br#"<?xml version="1.0"?><!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd"><html xmlns="http://www.w3.org/1999/xhtml"><head><title>One</title><link rel="stylesheet" type="text/css" href="../styles/book.css"/></head><body><p>fixture-body-one-7cb4</p></body></html>"#.to_vec()
    } else {
        br#"<html xmlns="http://www.w3.org/1999/xhtml"><body>One</body></html>"#.to_vec()
    };
    let one = if jpeg_cover {
        String::from_utf8(one)
            .expect("UTF-8 section")
            .replace("images/cover.png", "images/cover.jpg")
            .into_bytes()
    } else {
        one
    };
    let two = if epub2 || matches!(variant, EpubVariant::Epub3NcxFallback) {
        br#"<?xml version="1.0"?><!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd"><html xmlns="http://www.w3.org/1999/xhtml"><head><title>Two</title></head><body><p id="start">fixture-body-two-7cb4</p></body></html>"#.to_vec()
    } else {
        br#"<html xmlns="http://www.w3.org/1999/xhtml"><body id="start">Two</body></html>"#.to_vec()
    };
    let stylesheet = if matches!(variant, EpubVariant::MissingOptionalResources) {
        b"body { color: black; background-image: url('../images/missing.png'); }".as_slice()
    } else {
        b"body { color: black; }".as_slice()
    };
    let cover = match variant {
        EpubVariant::OrientedImage => oriented_jpeg(),
        EpubVariant::MalformedJpegImage => jpeg_without_scan_marker(),
        EpubVariant::MalformedExifImage => jpeg_with_malformed_exif(),
        EpubVariant::LateExifPngImage => png_with_late_exif(),
        EpubVariant::OversizedImage => oversized_png(),
        _ => PNG_1X1.to_vec(),
    };
    let cover_path = if jpeg_cover {
        "OEBPS/images/cover.jpg"
    } else {
        "OEBPS/images/cover.png"
    };
    for (name, bytes) in [
        ("META-INF/container.xml", container.as_slice()),
        ("OEBPS/book.opf", package.as_slice()),
        (nav_path, navigation.as_slice()),
        (one_path, one.as_slice()),
        (two_path, two.as_slice()),
        ("OEBPS/styles/book.css", stylesheet),
        (cover_path, cover.as_slice()),
    ] {
        archive.start_file(name, stored).expect("start epub member");
        archive.write_all(bytes).expect("write epub member");
    }
    if matches!(variant, EpubVariant::Changed) {
        archive
            .start_file("OEBPS/unused.bin", stored)
            .expect("changed member");
        archive.write_all(b"changed").expect("write changed member");
    }
    if matches!(
        variant,
        EpubVariant::FontObfuscation
            | EpubVariant::FontObfuscationContent
            | EpubVariant::FontObfuscationUnknown
    ) {
        archive
            .start_file("OEBPS/fonts/book.ttf", stored)
            .expect("font member");
        archive.write_all(b"obfuscated-font").expect("write font");
    } else if matches!(variant, EpubVariant::LargeOptionalFont) {
        let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        archive
            .start_file("OEBPS/fonts/book.ttf", deflated)
            .expect("large font member");
        let chunk = [0_u8; 8 * 1024];
        for _ in 0..(16 * 1024 * 1024 / chunk.len() + 1) {
            archive.write_all(&chunk).expect("write large font");
        }
    }
    archive.finish().expect("finish epub");
}

fn write_many_section_epub(path: &Path, count: usize) {
    let file = File::create(path).expect("create many-section epub");
    let mut archive = ZipWriter::new(file);
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    archive.start_file("mimetype", stored).expect("mimetype");
    archive
        .write_all(b"application/epub+zip")
        .expect("write mimetype");
    archive
        .start_file("META-INF/container.xml", stored)
        .expect("container");
    archive
        .write_all(br#"<?xml version="1.0"?><container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0"><rootfiles><rootfile full-path="OPS/book.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#)
        .expect("write container");

    let mut manifest = String::new();
    let mut spine = String::new();
    for index in 1..=count {
        let suffix = if index == count { "" } else { ".xhtml" };
        manifest.push_str(&format!(
            r#"<item id="item-{index}" href="text/{index}{suffix}" media-type="application/xhtml+xml"/>"#
        ));
        spine.push_str(&format!(r#"<itemref idref="item-{index}"/>"#));
    }
    let package = format!(
        r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata/><manifest>{manifest}</manifest><spine>{spine}</spine></package>"#
    );
    archive.start_file("OPS/book.opf", stored).expect("package");
    archive
        .write_all(package.as_bytes())
        .expect("write package");
    for index in 1..=count {
        let suffix = if index == count { "" } else { ".xhtml" };
        archive
            .start_file(format!("OPS/text/{index}{suffix}"), stored)
            .expect("section");
        archive
            .write_all(b"<html xmlns=\"http://www.w3.org/1999/xhtml\"><body>Section</body></html>")
            .expect("write section");
    }
    archive.finish().expect("finish many-section epub");
}

fn epub2_package(variant: EpubVariant) -> Vec<u8> {
    let mut package = r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" xmlns:dc="http://purl.org/dc/elements/1.1/" unique-identifier="book-id" version="2.0"><metadata><dc:title> Example   EPUB2 </dc:title><dc:creator>Legacy Author</dc:creator><dc:identifier id="book-id">urn:uuid:00000000-0000-0000-0000-000000000002</dc:identifier><dc:language>en</dc:language><meta name="cover" content="cover"/></metadata><manifest><item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/><item id="one" href="text/one.xhtml" media-type="application/xhtml+xml"/><item id="two" href="text/two.xhtml" media-type="application/xhtml+xml"/><item id="css" href="styles/book.css" media-type="text/css"/><item id="cover" href="images/cover.png" media-type="image/png"/></manifest><spine toc="ncx"><itemref idref="one"/><itemref idref="two"/></spine><guide><reference type="text" title="Start" href="text/one.xhtml"/></guide></package>"#.to_owned();
    match variant {
        EpubVariant::Epub2MissingSpineToc => {
            package = package.replace("<spine toc=\"ncx\">", "<spine>");
        }
        EpubVariant::Epub2MissingCoverItem => {
            package = package.replace("content=\"cover\"", "content=\"missing\"");
        }
        EpubVariant::Epub2UnsupportedCover => {
            package = package.replace("content=\"cover\"", "content=\"css\"");
        }
        _ => {}
    }
    package.into_bytes()
}

fn epub2_navigation_case(variant: EpubVariant) -> Vec<u8> {
    if matches!(
        variant,
        EpubVariant::Epub2NcxTocLimit | EpubVariant::Epub2NcxTocOverflow
    ) {
        let count = if matches!(variant, EpubVariant::Epub2NcxTocLimit) {
            2_000
        } else {
            2_001
        };
        let mut points = String::new();
        for index in 1..=count {
            points.push_str(&format!(
                r#"<navPoint id="point-{index}" playOrder="{index}"><navLabel><text>Point {index}</text></navLabel><content src="text/one.xhtml#point-{index}"/></navPoint>"#
            ));
        }
        return format!(
            r#"<?xml version="1.0"?><!DOCTYPE ncx PUBLIC "-//NISO//DTD ncx 2005-1//EN" "http://www.daisy.org/z3986/2005/ncx-2005-1.dtd"><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><head><meta name="dtb:uid" content="fixture"/></head><docTitle><text>Fixture</text></docTitle><navMap>{points}</navMap></ncx>"#
        )
        .into_bytes();
    }

    const HEAD: &str = r#"<head><meta name="dtb:uid" content="urn:uuid:00000000-0000-0000-0000-000000000002"/><meta name="dtb:depth" content="2"/><meta name="dtb:totalPageCount" content="0"/><meta name="dtb:maxPageNumber" content="0"/></head>"#;
    const TITLE: &str = "<docTitle><text>Example EPUB2</text></docTitle>";
    const LABELS: &str = r#"<navLabel xml:lang="en"><text> Part One </text></navLabel><navLabel xml:lang="zh"><text>Ignored alternate</text></navLabel>"#;
    let mut navigation = format!(
        r#"<?xml version="1.0"?><!DOCTYPE ncx PUBLIC "-//NISO//DTD ncx 2005-1//EN" "http://www.daisy.org/z3986/2005/ncx-2005-1.dtd"><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">{HEAD}{TITLE}<navMap><navPoint id="one" playOrder="1">{LABELS}<content src="text/one.xhtml"/><navPoint id="two" playOrder="2"><navLabel><text>Nested Two</text></navLabel><content src="text/two.xhtml#start"/></navPoint></navPoint></navMap></ncx>"#
    );
    match variant {
        EpubVariant::Epub2NcxIgnoredRootContent => {
            navigation = navigation
                .replace(
                    TITLE,
                    &format!("{TITLE}<docAuthor><text>Ignored author</text></docAuthor>"),
                )
                .replace(
                    "</navMap>",
                    r#"</navMap><pageList><navLabel><text>Pages</text></navLabel><pageTarget id="page-1" value="1" type="normal" playOrder="3"><navLabel><text>1</text></navLabel><content src="text/one.xhtml"/></pageTarget></pageList><navList class="other"><navLabel><text>Other</text></navLabel><navTarget id="other-1" playOrder="4"><navLabel><text>Other</text></navLabel><content src="text/two.xhtml"/></navTarget></navList>"#,
                );
        }
        EpubVariant::Epub2NcxAlternateLabel => {
            navigation = navigation.replace(
                LABELS,
                r#"<navLabel><text> </text></navLabel><navLabel><text>Part One</text></navLabel>"#,
            );
        }
        EpubVariant::Epub2NcxMissingHead => navigation = navigation.replace(HEAD, ""),
        EpubVariant::Epub2NcxDuplicateHead => {
            navigation = navigation.replace(HEAD, &format!("{HEAD}{HEAD}"));
        }
        EpubVariant::Epub2NcxMissingDocTitle => navigation = navigation.replace(TITLE, ""),
        EpubVariant::Epub2NcxDuplicateDocTitle => {
            navigation = navigation.replace(TITLE, &format!("{TITLE}{TITLE}"));
        }
        EpubVariant::Epub2NcxRootOutOfOrder => {
            navigation = navigation
                .replace(TITLE, "")
                .replace("</navMap>", &format!("</navMap>{TITLE}"));
        }
        EpubVariant::Epub2NcxUnknownRoot => {
            navigation = navigation.replace(TITLE, &format!("{TITLE}<unknown/>"));
        }
        EpubVariant::Epub2NcxDuplicateId => {
            navigation = navigation.replace("id=\"two\"", "id=\"one\"");
        }
        EpubVariant::Epub2NcxDuplicateHref => {
            navigation =
                navigation.replace("src=\"text/two.xhtml#start\"", "src=\"text/one.xhtml\"");
        }
        EpubVariant::Epub2NcxDuplicatePlayOrder => {
            navigation = navigation.replace("playOrder=\"2\"", "playOrder=\"1\"");
        }
        EpubVariant::Epub2NcxEmptyLabel => {
            navigation = navigation.replace(LABELS, "<navLabel><text>   </text></navLabel>");
        }
        EpubVariant::Epub2NcxOversizeLabel => {
            navigation = navigation.replace(
                LABELS,
                &format!("<navLabel><text>{}</text></navLabel>", "x".repeat(257)),
            );
        }
        EpubVariant::Epub2NcxWrongNamespace => {
            navigation =
                navigation.replace("http://www.daisy.org/z3986/2005/ncx/", "urn:invalid:ncx");
        }
        EpubVariant::Epub2NcxWrongVersion => {
            navigation = navigation.replace("version=\"2005-1\"", "version=\"2005-2\"");
        }
        _ => unreachable!("EPUB2 navigation case"),
    }
    navigation.into_bytes()
}

fn nested_elements(count: usize, empty_leaf: bool) -> String {
    let mut xml = "<x>".repeat(count);
    if empty_leaf {
        xml.push_str("<x/>");
    }
    xml.push_str(&"</x>".repeat(count));
    xml
}
