use hw_parser::{lookup_pnp_manufacturer, parse_xrandr_query, parse_xrandr_verbose};

#[test]
fn parse_xrandr_query_extracts_first_mode_as_max_resolution() {
    let records = parse_xrandr_query(
        "HDMI-1 connected primary 1920x1080+0+0\n\
           2560x1440     59.95 +\n\
           1920x1080     60.00*\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].resolution.as_deref(), Some("1920x1080"));
    assert_eq!(records[0].max_resolution.as_deref(), Some("2560x1440"));
}

#[test]
fn parse_xrandr_verbose_extracts_edid_bytes_by_connector() {
    let records = parse_xrandr_verbose(
        "HDMI-1 connected primary 1920x1080+0+0\n\
        \tEDID:\n\
        \t\t00ffffffffffff0005e3341200000000\n\
        \t\t0c200103803420780000000000000000\n\
        eDP-1 disconnected\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].connector, "HDMI-1");
    assert_eq!(
        &records[0].edid[0..8],
        &[0, 255, 255, 255, 255, 255, 255, 0]
    );
}

#[test]
fn parse_xrandr_verbose_stops_edid_at_non_indented_hex_line() {
    let records = parse_xrandr_verbose(
        "HDMI-1 connected primary 1920x1080+0+0\n\
        \tEDID:\n\
        \t\t00ff\n\
        1122\n",
    );

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].edid, vec![0, 255]);
}

#[test]
fn parse_xrandr_verbose_suppresses_odd_length_edid() {
    let records = parse_xrandr_verbose(
        "HDMI-1 connected primary 1920x1080+0+0\n\
        \tEDID:\n\
        \t\t00f\n",
    );

    assert!(records.is_empty());
}

#[test]
fn parse_xrandr_verbose_ignores_disconnected_edid() {
    let records = parse_xrandr_verbose(
        "HDMI-1 disconnected\n\
        \tEDID:\n\
        \t\t00ff\n",
    );

    assert!(records.is_empty());
}

#[test]
fn parse_xrandr_verbose_ignores_empty_edid() {
    let records = parse_xrandr_verbose(
        "HDMI-1 connected primary 1920x1080+0+0\n\
        \tEDID:\n\
        \n",
    );

    assert!(records.is_empty());
}

#[test]
fn pnp_lookup_returns_known_manufacturer_names() {
    assert_eq!(lookup_pnp_manufacturer("AOC"), Some("AOC International"));
    assert_eq!(lookup_pnp_manufacturer(" aoc "), Some("AOC International"));
    assert_eq!(lookup_pnp_manufacturer("ZZZ"), None);
}
