use hw_parser::parse_dmi_oem_strings;
use hw_testdata::fixture;

#[test]
fn dmi_oem_strings_are_parsed_and_filtered() {
    let input = fixture("dmi/dmidecode-t11.txt");
    let strings = parse_dmi_oem_strings(&input);
    assert_eq!(
        strings,
        vec![
            "Default string".to_string(),
            "LENOVO_MT_20UAS0LK00_BU_Think_FM_ThinkPad X1 Carbon Gen 9".to_string(),
            "LENOVO_BIOS: N32ET75W (1.50 )".to_string(),
        ]
    );
    // "Not Specified" is filtered out.
}

#[test]
fn dmi_oem_strings_returns_empty_when_section_missing() {
    assert!(parse_dmi_oem_strings(
        "Handle 0x0001, DMI type 0, 20 bytes\nBIOS Information"
    )
    .is_empty());
}
