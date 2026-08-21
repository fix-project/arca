include!(concat!(env!("OUT_DIR"), "/artifacts.rs"));

pub fn artifact(name: &str) -> Option<&'static [u8]> {
    ARTIFACTS
        .iter()
        .find_map(|(artifact_name, bytes)| (*artifact_name == name).then_some(*bytes))
}
