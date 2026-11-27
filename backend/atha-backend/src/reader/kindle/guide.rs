use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{BufReader, Read},
    path::Path,
};

use quick_xml::{Reader, XmlVersion, events::Event};

use super::{
    ImportError, MAX_MANIFEST_TOC_ITEMS, MAX_RESOURCE_BYTES, SECTION_PREFIX, local_name,
    valid_fragment,
};

pub(super) const TARGET_ATTRIBUTE: &str = "data-atha-guide-href";

#[derive(Default)]
pub(super) struct Targets {
    destinations: HashMap<String, HashSet<String>>,
    count: usize,
}

impl Targets {
    pub(super) fn project(
        &mut self,
        href: &str,
        source: &str,
        spine: &HashMap<String, String>,
    ) -> Result<String, ImportError> {
        let (path, fragment) = href.split_once('#').ok_or(ImportError::InvalidReference)?;
        if !valid_fragment(fragment) {
            return Err(ImportError::InvalidReference);
        }
        let target = spine
            .get(if path.is_empty() {
                source
            } else {
                path.trim_start_matches("./")
            })
            .ok_or(ImportError::InvalidReference)?;
        let relative = target
            .strip_prefix(SECTION_PREFIX)
            .ok_or(ImportError::InvalidReference)?;
        let fragments = self.destinations.entry(target.clone()).or_default();
        if !fragments.contains(fragment) {
            if self.count == MAX_MANIFEST_TOC_ITEMS {
                return Err(ImportError::TooManyTocItems);
            }
            fragments.insert(fragment.to_owned());
            self.count += 1;
        }
        if path.is_empty() {
            Ok(href.to_owned())
        } else {
            Ok(format!("{relative}#{fragment}"))
        }
    }

    pub(super) fn verify(self, staging: &Path) -> Result<(), ImportError> {
        self.verify_with(|path| {
            File::open(staging.join(path)).map_err(|_| ImportError::WriteFailed)
        })
    }

    fn verify_with<R: Read>(
        self,
        mut open: impl FnMut(&str) -> Result<R, ImportError>,
    ) -> Result<(), ImportError> {
        // Only inspect chapters named by guide references, once per target.
        // Read the projected XHTML, not boko's untrusted raw markup.
        for (path, mut missing) in self.destinations {
            find_body_anchors(open(&path)?, &mut missing)?;
            if !missing.is_empty() {
                return Err(ImportError::InvalidReference);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guide_targets_require_real_body_anchors_in_the_named_chapter() {
        let spine = HashMap::from([
            (
                "one.html".to_owned(),
                ".atha-kindle/section-0001.xhtml".to_owned(),
            ),
            (
                "two.html".to_owned(),
                ".atha-kindle/section-0002.xhtml".to_owned(),
            ),
        ]);
        let open = |path: &str| -> Result<&'static [u8], ImportError> {
            match path {
                ".atha-kindle/section-0001.xhtml" => Ok(br#"<html><head><meta id="head-only"/></head><body><a name="first">One</a></body></html>"#),
                ".atha-kindle/section-0002.xhtml" => Ok(br#"<html><head/><body><h2 id="second">Two</h2></body></html>"#),
                _ => Err(ImportError::WriteFailed),
            }
        };
        let mut projected = Targets::default();
        assert_eq!(
            projected.project("#first", "one.html", &spine).unwrap(),
            "#first"
        );
        assert_eq!(
            projected
                .project("two.html#second", "one.html", &spine)
                .unwrap(),
            "section-0002.xhtml#second"
        );
        assert_eq!(projected.verify_with(open), Ok(()));

        for bad in ["#missing", "#head-only", "two.html#first"] {
            let mut projected = Targets::default();
            projected.project(bad, "one.html", &spine).unwrap();
            assert_eq!(
                projected.verify_with(open),
                Err(ImportError::InvalidReference)
            );
        }
    }
}

fn find_body_anchors(input: impl Read, missing: &mut HashSet<String>) -> Result<(), ImportError> {
    let mut reader = Reader::from_reader(BufReader::new(input.take(MAX_RESOURCE_BYTES)));
    let mut buffer = Vec::new();
    let mut body_depth = None;
    let mut depth = 0_usize;
    loop {
        let event = reader
            .read_event_into(&mut buffer)
            .map_err(|_| ImportError::InvalidMarkup)?;
        match event {
            Event::Start(ref start) | Event::Empty(ref start) => {
                let name = local_name(start.name().as_ref())?;
                let container = matches!(event, Event::Start(_));
                if depth == 1 && name == "body" && container {
                    body_depth = Some(depth + 1);
                }
                if body_depth.is_some() {
                    for attribute in start.attributes().with_checks(true) {
                        let attribute = attribute.map_err(|_| ImportError::InvalidMarkup)?;
                        let key = attribute.key.as_ref();
                        if key == b"id" || (name == "a" && key == b"name") {
                            let value = attribute
                                .decoded_and_normalized_value(
                                    XmlVersion::Implicit1_0,
                                    reader.decoder(),
                                )
                                .map_err(|_| ImportError::InvalidMarkup)?;
                            missing.remove(value.as_ref());
                        }
                    }
                    if missing.is_empty() {
                        return Ok(());
                    }
                }
                if container {
                    depth += 1;
                }
            }
            Event::End(_) => {
                if body_depth == Some(depth) {
                    return Ok(());
                }
                depth = depth.checked_sub(1).ok_or(ImportError::InvalidMarkup)?;
            }
            Event::Eof => return Ok(()),
            _ => {}
        }
        buffer.clear();
    }
}
