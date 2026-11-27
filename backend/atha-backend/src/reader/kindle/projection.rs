//! Pre-allocation output bounds for the extended HUFF path and pinned boko 0.5.0.
//! Keep splitting and link/anchor generation in boko; count only expansion sites.

use std::io::{Read, Seek, SeekFrom};

use super::{ImportError, MAX_RESOURCE_BYTES, MAX_TEXT_BYTES};

pub(super) enum Layout {
    Mobi { ncx: usize },
    Kf8(Vec<Part>),
}

pub(super) struct Part {
    pub start: usize,
    pub bytes: usize,
    pub insertions: usize,
}

impl Layout {
    pub(super) fn verify_projection(
        self,
        stage: &mut (impl Read + Seek),
        text_offset: u64,
        text_bytes: usize,
        record_zero_bytes: usize,
    ) -> Result<(), ImportError> {
        let mut budget = Budget {
            section: MAX_RESOURCE_BYTES,
            remaining: MAX_TEXT_BYTES,
        };
        match self {
            Self::Mobi { ncx } => {
                let plain = scan(
                    stage,
                    text_offset,
                    text_bytes,
                    [b"<", b"filepos=", b"recindex=\""],
                )?;
                bound_mobi(
                    &mut budget,
                    plain,
                    text_bytes as u64,
                    record_zero_bytes as u64,
                    ncx as u64,
                )
            }
            Self::Kf8(parts) => {
                for part in parts {
                    let scan = scan(
                        stage,
                        text_offset + part.start as u64,
                        part.bytes,
                        [b"kindle:", b"kindle:pos:fid:"],
                    )?;
                    bound_kf8(
                        &mut budget,
                        part.bytes as u64,
                        scan.tokens,
                        part.insertions as u64,
                    )?;
                }
                Ok(())
            }
        }
    }
}

struct Budget {
    section: u64,
    remaining: u64,
}

impl Budget {
    fn charge(&mut self, largest_section: u64, total: u64) -> Result<(), ImportError> {
        if largest_section > self.section {
            return Err(ImportError::TextTooLarge);
        }
        self.remaining = self
            .remaining
            .checked_sub(total)
            .ok_or(ImportError::TextTooLarge)?;
        Ok(())
    }
}

fn bound_kf8(
    budget: &mut Budget,
    bytes: u64,
    [refs, fragments]: [u64; 2],
    insertions: u64,
) -> Result<(), ImportError> {
    // A DIV insertion creates up to two joins. Neither prefix overlaps
    // with itself, so each join can add at most one candidate of each kind.
    let joins = 2 * insertions;
    // Normal replacement paths fit in 64 bytes. Position links may also
    // carry an ASCII anchor from boko's 2,000-byte search window.
    let rewritten = bytes + 64 * (refs + joins) + 2048 * (fragments + joins);
    // Attribute cleanup adds seven bytes to some <img ...> tags (six
    // input bytes minimum, hence <=3x). Solidus repair adds at most
    // one space per slash moved (<=2x). boko then caches the result.
    let projected = 6 * rewritten;
    budget.charge(projected, projected)
}

fn bound_mobi(
    budget: &mut Budget,
    plain: Scan<3>,
    bytes: u64,
    record_zero: u64,
    ncx: u64,
) -> Result<(), ImportError> {
    let (wrapped, [openers, links, images]) = if plain.full_document {
        (bytes, plain.tokens)
    } else {
        // A UTF-8 conversion can emit three bytes per input byte; the
        // escaped title can emit another six per converted byte from
        // record zero (or the 32-byte PDB name). The fixed wrapper fits 256.
        let max = 3 * bytes + 18 * record_zero.max(32) + 256;
        (max, [max, max / 8, max / 10])
    };

    // Each "filepos=" candidate can add one short <a id> to the text and
    // one href replacement; "recindex=\"" can add one asset path. Inserted
    // anchors contain neither candidate prefix and cannot create one across
    // an insertion. Solidus repair (<=2x), then UTF-8 conversion (<=3x).
    let transformed = 6 * (wrapped + links * (32 + 40) + images * 48);
    // Every pagebreak uses a distinct '<'; only the inserted anchors can
    // add any over the wrapped input. Attempt the ordinary split first.
    charge_split(budget, transformed, openers + links + 1)?;
    if ncx != 0 {
        // The fallback can add at most one 32-byte anchor and one split
        // point per NCX entry. Charge it too: boko retains the first result
        // during this attempt, even if the fallback is eventually unused.
        charge_split(budget, transformed + 6 * 32 * ncx, ncx + 1)?;
    }
    Ok(())
}

fn charge_split(budget: &mut Budget, transformed: u64, chapters: u64) -> Result<(), ImportError> {
    // Body chunks are disjoint. We bound the shared head by the entire
    // transformed input, not by guessing where boko will find its tags.
    // Every chapter can also have 256 bytes of XHTML wrapper. Cross-chapter
    // links add at most 34 bytes of path per >=14-byte "href=\"#filepos"
    // occurrence, hence <=4x, and bare-filename cleanup cannot grow output.
    let copied_head_and_wrapper = transformed + 256;
    let section = 4 * (transformed + copied_head_and_wrapper);
    let total = copied_head_and_wrapper
        .checked_mul(chapters)
        .and_then(|head| head.checked_add(transformed))
        .and_then(|all| all.checked_mul(4))
        .ok_or(ImportError::TextTooLarge)?;
    budget.charge(section, total)
}

struct Scan<const N: usize> {
    tokens: [u64; N],
    full_document: bool,
}

fn scan<const N: usize>(
    input: &mut (impl Read + Seek),
    offset: u64,
    mut left: usize,
    patterns: [&[u8]; N],
) -> Result<Scan<N>, ImportError> {
    input
        .seek(SeekFrom::Start(offset))
        .map_err(|_| ImportError::WriteFailed)?;
    let mut result = Scan {
        tokens: [0; N],
        full_document: false,
    };
    let mut buffer = [0; 16 * 1024 + 16];
    let keep = patterns.iter().map(|p| p.len() - 1).max().unwrap_or(0);
    let mut carry = 0;
    let mut first = true;
    while left != 0 {
        let take = left.min(buffer.len() - carry);
        input
            .read_exact(&mut buffer[carry..carry + take])
            .map_err(|_| ImportError::WriteFailed)?;
        let end = carry + take;
        let bytes = &buffer[..end];
        if first {
            // An ASCII prefix (with no BOM or non-ASCII whitespace) proves
            // boko's no-wrap branch under either admitted encoding. In all
            // other cases use the bounded worst-case wrapper calculation.
            let prefix = bytes.trim_ascii_start();
            result.full_document = prefix.starts_with(b"<html") || prefix.starts_with(b"<!DOCTYPE");
            first = false;
        }
        for (pattern, count) in patterns.iter().zip(&mut result.tokens) {
            let already_counted = carry.saturating_sub(pattern.len() - 1);
            *count += bytes[already_counted..]
                .windows(pattern.len())
                .filter(|w| w == pattern)
                .count() as u64;
        }
        carry = end.min(keep);
        buffer.copy_within(end - carry..end, 0);
        left -= take;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn normal_kf8_fragment_and_image_output_are_charged_before_rendering() {
        let raw =
            br#"<p><a href="kindle:pos:fid:0:off:0">Chapter</a><img src="kindle:embed:1"/></p>"#;
        let mut input = Cursor::new(raw);
        let stats = scan(&mut input, 0, raw.len(), [b"kindle:", b"kindle:pos:fid:"]).unwrap();
        assert_eq!(stats.tokens, [2, 1]);

        let projected = 6 * (raw.len() as u64 + 64 * 2 + 2048);
        let mut budget = Budget {
            section: projected,
            remaining: projected,
        };
        assert_eq!(
            bound_kf8(&mut budget, raw.len() as u64, stats.tokens, 0),
            Ok(())
        );
        assert_eq!(budget.remaining, 0);
        assert_eq!(
            bound_kf8(&mut budget, 1, [0, 0], 0),
            Err(ImportError::TextTooLarge)
        );
        let mut seam_budget = Budget {
            section: projected,
            remaining: projected,
        };
        assert_eq!(
            bound_kf8(&mut seam_budget, raw.len() as u64, stats.tokens, 1),
            Err(ImportError::TextTooLarge)
        );
    }

    #[test]
    fn legacy_head_budget_counts_pagebreak_and_ncx_fallback_chapters() {
        let markup = b"<html><head><title>Gate</title></head><body><p>First</p><mbp:pagebreak/><p>Next</p></body></html>";
        let scan_mobi = || {
            scan(
                &mut Cursor::new(markup),
                0,
                markup.len(),
                [b"<", b"filepos=", b"recindex=\""],
            )
            .unwrap()
        };
        let plain = scan_mobi();
        assert!(plain.full_document);
        let bytes = markup.len() as u64;
        let rewritten = bytes * 6;
        let ordinary = 4 * (rewritten + (rewritten + 256) * (plain.tokens[0] + 1));
        let fallback = rewritten + 6 * 32;
        let both = ordinary + 4 * (fallback + 2 * (fallback + 256));
        let check = |limit, ncx| {
            bound_mobi(
                &mut Budget {
                    section: 10_000,
                    remaining: limit,
                },
                scan_mobi(),
                bytes,
                244,
                ncx,
            )
        };
        assert_eq!(check(ordinary, 0), Ok(()));
        assert_eq!(check(ordinary - 1, 0), Err(ImportError::TextTooLarge));
        assert_eq!(check(both, 1), Ok(()));
        assert_eq!(check(both - 1, 1), Err(ImportError::TextTooLarge));
    }

    #[test]
    fn stream_counts_markers_that_span_a_buffer_boundary_once() {
        let mut text = vec![b' '; 16 * 1024 + 14];
        text.extend_from_slice(b"kindle:pos:fid:");
        let stats = scan(
            &mut Cursor::new(&text),
            0,
            text.len(),
            [b"kindle:", b"kindle:pos:fid:"],
        )
        .unwrap();
        assert_eq!(stats.tokens, [1, 1]);
    }
}
