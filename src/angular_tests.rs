use super::*;

fn env(values: &[(&str, &str)]) -> Vec<(String, String)> {
    values
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

#[test]
fn finds_macos_extension_work_directory() {
    assert_eq!(
        AngularExtension::managed_extension_dir_for(Os::Mac, &env(&[("HOME", "/Users/ruimonte")]),),
        Ok("/Users/ruimonte/Library/Application Support/Zed/extensions/work/angular".into())
    );
}

#[test]
fn finds_linux_extension_work_directory() {
    assert_eq!(
        AngularExtension::managed_extension_dir_for(
            Os::Linux,
            &env(&[("XDG_DATA_HOME", "/var/lib/user")]),
        ),
        Ok("/var/lib/user/zed/extensions/work/angular".into())
    );
}

#[test]
fn finds_windows_extension_work_directory() {
    assert_eq!(
        AngularExtension::managed_extension_dir_for(
            Os::Windows,
            &env(&[("LOCALAPPDATA", r"C:\Users\rui\AppData\Local")]),
        ),
        Ok("C:/Users/rui/AppData/Local/Zed/extensions/work/angular".into())
    );
}
