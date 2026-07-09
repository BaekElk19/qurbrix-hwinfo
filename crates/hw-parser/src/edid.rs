#[derive(Debug, Clone, PartialEq)]
pub struct EdidRecord {
    pub manufacturer: Option<String>,
    pub product_code: Option<u16>,
    pub serial: Option<String>,
    pub name: Option<String>,
    pub week: Option<u8>,
    pub year: Option<u16>,
    pub size_cm: Option<(u8, u8)>,
    pub size_mm: Option<(u32, u32)>,
    pub gamma: Option<f32>,
    pub preferred_mode: Option<PreferredMode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreferredMode {
    pub width: u16,
    pub height: u16,
    pub refresh_hz: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdidError {
    TooShort,
    BadHeader,
    BadChecksum,
}

const EDID_HEADER: [u8; 8] = [0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00];
const PAIR_SWAPPED_EDID_HEADER: [u8; 8] = [0xff, 0x00, 0xff, 0xff, 0xff, 0xff, 0x00, 0xff];

pub fn parse_edid(bytes: &[u8]) -> Result<EdidRecord, EdidError> {
    if bytes.len() < 128 {
        return Err(EdidError::TooShort);
    }

    let normalized;
    let block = if bytes[..8] == PAIR_SWAPPED_EDID_HEADER {
        normalized = pair_swap_block(&bytes[..128]);
        normalized.as_slice()
    } else {
        &bytes[..128]
    };
    if block[..8] != EDID_HEADER {
        return Err(EdidError::BadHeader);
    }
    if block.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte)) != 0 {
        return Err(EdidError::BadChecksum);
    }

    let mut name = None;
    let mut alphanumeric_name = None;
    let mut serial = None;
    for offset in [54, 72, 90, 108] {
        if let Some((tag, text)) = parse_text_descriptor(&block[offset..offset + 18]) {
            match tag {
                0xfc => name = Some(text),
                0xfe => alphanumeric_name = Some(text),
                0xff => serial = Some(text),
                _ => {}
            }
        }
    }
    let size_cm = (block[21] != 0 && block[22] != 0).then_some((block[21], block[22]));

    Ok(EdidRecord {
        manufacturer: parse_manufacturer(block[8], block[9]),
        product_code: Some(u16::from_le_bytes([block[10], block[11]])),
        serial,
        name: name.or(alphanumeric_name),
        week: Some(block[16]),
        year: Some(1990 + block[17] as u16),
        size_cm,
        size_mm: physical_size_mm(block, size_cm),
        gamma: parse_gamma(block[23]),
        preferred_mode: parse_preferred_mode(&block[54..72]),
    })
}

fn pair_swap_block(block: &[u8]) -> Vec<u8> {
    block
        .chunks(2)
        .flat_map(|chunk| [chunk[1], chunk[0]])
        .collect()
}

fn parse_manufacturer(msb: u8, lsb: u8) -> Option<String> {
    let value = u16::from_be_bytes([msb, lsb]);
    let mut id = String::with_capacity(3);
    for shift in [10, 5, 0] {
        let c = ((value >> shift) & 0x1f) as u8;
        if !(1..=26).contains(&c) {
            return None;
        }
        id.push((b'A' + c - 1) as char);
    }
    Some(id)
}

fn parse_preferred_mode(desc: &[u8]) -> Option<PreferredMode> {
    let pixel_clock = u16::from_le_bytes([desc[0], desc[1]]);
    if pixel_clock == 0 {
        return None;
    }

    let width = desc[2] as u16 | (((desc[4] >> 4) as u16) << 8);
    let h_blank = desc[3] as u16 | (((desc[4] & 0x0f) as u16) << 8);
    let height = desc[5] as u16 | (((desc[7] >> 4) as u16) << 8);
    let v_blank = desc[6] as u16 | (((desc[7] & 0x0f) as u16) << 8);

    if width == 0 || height == 0 {
        return None;
    }

    Some(PreferredMode {
        width,
        height,
        refresh_hz: refresh_hz(pixel_clock, width + h_blank, height + v_blank),
    })
}

fn physical_size_mm(block: &[u8], base_size_cm: Option<(u8, u8)>) -> Option<(u32, u32)> {
    let base_size_mm = base_size_cm.map(|(width, height)| (width as u32 * 10, height as u32 * 10));
    let dtd_size = [54, 72, 90, 108]
        .into_iter()
        .filter_map(|offset| parse_dtd_size_mm(&block[offset..offset + 18]))
        .find(|size| {
            let Some((base_width, base_height)) = base_size_mm else {
                return true;
            };
            size.0.abs_diff(base_width) < 10 && size.1.abs_diff(base_height) < 10
        });
    dtd_size.or(base_size_mm)
}

fn parse_dtd_size_mm(desc: &[u8]) -> Option<(u32, u32)> {
    let pixel_clock = u16::from_le_bytes([desc[0], desc[1]]);
    if pixel_clock == 0 {
        return None;
    }
    let width = desc[12] as u16 | (((desc[14] >> 4) as u16) << 8);
    let height = desc[13] as u16 | (((desc[14] & 0x0f) as u16) << 8);
    (width != 0 && height != 0).then_some((width as u32, height as u32))
}

fn parse_gamma(value: u8) -> Option<f32> {
    (value != 0xff).then_some((value as f32 + 100.0) / 100.0)
}

fn refresh_hz(pixel_clock: u16, h_total: u16, v_total: u16) -> u16 {
    let total_pixels = h_total as u32 * v_total as u32;
    if total_pixels == 0 {
        return 0;
    }

    ((pixel_clock as u32 * 10_000 + total_pixels / 2) / total_pixels) as u16
}

fn parse_text_descriptor(desc: &[u8]) -> Option<(u8, String)> {
    if desc[0] != 0 || desc[1] != 0 || desc[2] != 0 || desc[4] != 0 {
        return None;
    }

    let text = String::from_utf8_lossy(&desc[5..18])
        .trim_matches(['\0', '\n', '\r', ' '])
        .to_string();
    (!text.is_empty()).then_some((desc[3], text))
}
