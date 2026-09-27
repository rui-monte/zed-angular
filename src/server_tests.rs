use super::*;
use zed_extension_api::serde_json;

#[test]
fn managed_command_uses_only_the_pinned_stack_and_preserves_node_options() {
    let args = Server::managed("/zed data/extensions/work/angular/").arguments(Some(8192));
    assert_eq!(
        args,
        [
            "--max-old-space-size=8192",
            "/zed data/extensions/work/angular/node_modules/@angular/language-server/index.js",
            "--stdio",
            "--tsProbeLocations",
            "/zed data/extensions/work/angular",
            "--ngProbeLocations",
            "/zed data/extensions/work/angular",
            "--logToConsole",
            "--logVerbosity",
            "normal",
        ]
    );
}

#[test]
fn custom_command_keeps_project_first_and_nested_package_probes() {
    let args = Server::custom(
        "/repo",
        " client/node_modules/@angular/language-server/index.js ",
        &[],
    )
    .arguments(None);
    assert_eq!(
        args[0],
        "/repo/client/node_modules/@angular/language-server/index.js"
    );
    assert_eq!(args[3], "/repo,/repo/node_modules,/repo/client/node_modules/@angular,/repo/client/node_modules,/repo/client");
    assert_eq!(args[3], args[5]);
}

#[test]
fn custom_paths_accept_home_absolute_and_windows_locations() {
    let env = [("HOME".into(), "/home/user".into())];
    for (requested, expected) in [
        ("~/server/", "/home/user/server/index.js"),
        ("/opt/server/index.js", "/opt/server/index.js"),
        ("C:\\tools\\server\\", "C:/tools/server/index.js"),
    ] {
        assert_eq!(
            Server::custom("/repo", requested, &env).arguments(None)[0],
            expected
        );
    }
}

#[test]
#[ignore = "requires npm ci in managed-server/ and tests/"]
fn managed_server_runtime() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let args = Server::managed(root.join("managed-server").to_str().unwrap()).arguments(None);
    let output = std::process::Command::new("node")
        .args(["--test", "tests/managed-server.test.mjs"])
        .current_dir(root)
        .env(
            "ZED_ANGULAR_SERVER_ARGS",
            serde_json::to_string(&args).unwrap(),
        )
        .output()
        .expect("Node must be installed to run the language-server integration tests");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stdout}\n{stderr}");
    print!("{stdout}");
}
