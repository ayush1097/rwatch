use std::fs;

#[test]
fn rwatch_config_has_expected_values() {
    let contents = fs::read_to_string("rwatch.toml").expect("rwatch.toml should exist");

    assert!(contents.contains("refresh_interval"));
    assert!(contents.contains("process_interval"));
    assert!(contents.contains("network_interval"));
    assert!(contents.contains("log_level"));
}
