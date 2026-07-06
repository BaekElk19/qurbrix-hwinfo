use hw_parser::{parse_edid, EdidError};

fn sample_edid() -> Vec<u8> {
    let mut edid = vec![0u8; 128];
    edid[0..8].copy_from_slice(&[0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00]);
    edid[8] = 0x05;
    edid[9] = 0xe3; // AOC
    edid[10] = 0x34;
    edid[11] = 0x12;
    edid[16] = 12;
    edid[17] = 32; // 2022
    edid[21] = 52;
    edid[22] = 32;
    edid[23] = 120;
    write_dtd(&mut edid, 54, 14850, 1920, 312, 1080, 45);
    edid[72] = 0x00;
    edid[73] = 0x00;
    edid[74] = 0x00;
    edid[75] = 0xfc;
    edid[76] = 0x00;
    edid[77..90].copy_from_slice(b"AOC TEST    \n");
    update_checksum(&mut edid);
    edid
}

fn write_dtd(
    edid: &mut [u8],
    offset: usize,
    pixel_clock: u16,
    width: u16,
    h_blank: u16,
    height: u16,
    v_blank: u16,
) {
    edid[offset..offset + 2].copy_from_slice(&pixel_clock.to_le_bytes());
    edid[offset + 2] = width as u8;
    edid[offset + 3] = h_blank as u8;
    edid[offset + 4] = (((width >> 8) as u8) << 4) | ((h_blank >> 8) as u8 & 0x0f);
    edid[offset + 5] = height as u8;
    edid[offset + 6] = v_blank as u8;
    edid[offset + 7] = (((height >> 8) as u8) << 4) | ((v_blank >> 8) as u8 & 0x0f);
}

fn update_checksum(edid: &mut [u8]) {
    let checksum = (256u16 - edid[..127].iter().map(|b| *b as u16).sum::<u16>() % 256) % 256;
    edid[127] = checksum as u8;
}

#[test]
fn parse_edid_extracts_identity_and_timing() {
    let edid = parse_edid(&sample_edid()).unwrap();

    assert_eq!(edid.manufacturer.as_deref(), Some("AOC"));
    assert_eq!(edid.product_code, Some(0x1234));
    assert_eq!(edid.week, Some(12));
    assert_eq!(edid.year, Some(2022));
    assert_eq!(edid.size_cm, Some((52, 32)));
    assert_eq!(edid.gamma, Some(2.2));
    assert_eq!(edid.name.as_deref(), Some("AOC TEST"));
    let mode = edid.preferred_mode.unwrap();
    assert_eq!(mode.width, 1920);
    assert_eq!(mode.height, 1080);
}

#[test]
fn parse_edid_keeps_standard_dtd_dimensions() {
    let mut bytes = sample_edid();
    write_dtd(&mut bytes, 54, 29, 800, 0, 1056, 0);
    update_checksum(&mut bytes);

    let mode = parse_edid(&bytes).unwrap().preferred_mode.unwrap();

    assert_eq!(mode.width, 800);
    assert_eq!(mode.height, 1056);
}

#[test]
fn parse_edid_rejects_out_of_range_manufacturer_letters() {
    let mut bytes = sample_edid();
    bytes[8] = 0x6c;
    bytes[9] = 0x21; // 27, 1, 1 -> invalid, A, A
    update_checksum(&mut bytes);

    let edid = parse_edid(&bytes).unwrap();

    assert_eq!(edid.manufacturer, None);
}

#[test]
fn parse_edid_rejects_short_input() {
    assert_eq!(parse_edid(&[0; 127]).unwrap_err(), EdidError::TooShort);
}

#[test]
fn parse_edid_rejects_bad_header() {
    let mut bytes = sample_edid();
    bytes[0] = 0x01;

    assert_eq!(parse_edid(&bytes).unwrap_err(), EdidError::BadHeader);
}

#[test]
fn parse_edid_rejects_bad_checksum() {
    let mut bytes = sample_edid();
    bytes[127] = bytes[127].wrapping_add(1);

    assert_eq!(parse_edid(&bytes).unwrap_err(), EdidError::BadChecksum);
}

#[test]
fn parse_edid_extracts_descriptor_serial() {
    let mut bytes = sample_edid();
    bytes[90] = 0x00;
    bytes[91] = 0x00;
    bytes[92] = 0x00;
    bytes[93] = 0xff;
    bytes[94] = 0x00;
    bytes[95..108].copy_from_slice(b"SERIAL123   \n");
    update_checksum(&mut bytes);

    let edid = parse_edid(&bytes).unwrap();

    assert_eq!(edid.serial.as_deref(), Some("SERIAL123"));
}
