use fixcertificates::linux::read_config;
use fixrepository::Repository;

#[test]
fn autoconfigure_pins_the_bootstrap_programs() {
    let root = std::path::PathBuf::from(std::env::var("TEST_TMPDIR").unwrap());
    let path = root.join("data/fix/bootstrap/.fix");
    let config_path = root.join("config/fix");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let repository = Repository::init(path.to_str().unwrap()).unwrap().unsigned();
    let mut names = Vec::new();
    for (index, label) in ["scheduler", "collector", "fix-parser", "compile"]
        .into_iter()
        .enumerate()
    {
        let value = repository.create_blob(&[index as u8; 40]).unwrap();
        repository.write_label(label, value.into()).unwrap();
        names.push(value.as_handle().to_string());
    }

    unsafe { std::env::set_var("FIX_CONFIG_DIR", &config_path) };
    fixcli::configuration::autoconfigure(&repository).unwrap();
    let config = read_config(&config_path.join("config")).unwrap();
    assert_eq!(config.scheduler.as_deref(), Some(names[0].as_str()));
    assert_eq!(config.collector.as_deref(), Some(names[1].as_str()));
    assert_eq!(config.parser.as_deref(), Some(names[2].as_str()));
    assert_eq!(config.compiler.as_deref(), Some(names[3].as_str()));

    let replacement = repository.create_blob(&[42; 40]).unwrap();
    repository
        .write_label("scheduler", replacement.into())
        .unwrap();
    let config = read_config(&config_path.join("config")).unwrap();
    assert_eq!(config.scheduler.as_deref(), Some(names[0].as_str()));
}
