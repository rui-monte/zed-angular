use std::collections::BTreeMap;

use serde::Deserialize;
use zed::{LanguageServerId, LanguageServerInstallationStatus, Result};
use zed_extension_api::{self as zed, serde_json};

use crate::server::Server;

#[derive(Deserialize)]
struct Manifest {
    dependencies: BTreeMap<String, String>,
}

trait PackageManager {
    fn installed_version(&self, package: &str) -> Result<Option<String>>;
    fn install(&self, package: &str, version: &str) -> Result<()>;
}

struct Npm<'a> {
    language_server_id: &'a LanguageServerId,
}

impl PackageManager for Npm<'_> {
    fn installed_version(&self, package: &str) -> Result<Option<String>> {
        zed::npm_package_installed_version(package)
    }

    fn install(&self, package: &str, version: &str) -> Result<()> {
        zed::set_language_server_installation_status(
            self.language_server_id,
            &LanguageServerInstallationStatus::Downloading,
        );
        zed::npm_install_package(package, version)
    }
}

pub fn server(language_server_id: &LanguageServerId) -> Result<Server> {
    let result: Result<Server> = (|| {
        // Zed sets PWD inside the extension's WASI environment to the actual
        // npm work directory, including custom data directories and remote hosts.
        let directory = std::env::var("PWD")
            .map_err(|error| format!("Cannot locate Zed extension storage: {error}"))?;
        ensure_installed(&Npm { language_server_id })?;
        Ok(Server::managed(&directory))
    })();

    let status = match &result {
        Ok(_) => LanguageServerInstallationStatus::None,
        Err(error) => LanguageServerInstallationStatus::Failed(error.clone()),
    };
    zed::set_language_server_installation_status(language_server_id, &status);
    result
}

fn ensure_installed(npm: &impl PackageManager) -> Result<()> {
    let manifest: Manifest =
        serde_json::from_str(include_str!("../managed-server/package.json"))
            .map_err(|error| format!("Invalid managed server manifest: {error}"))?;

    for (package, version) in manifest.dependencies {
        let installed = npm
            .installed_version(&package)
            .map_err(|error| format!("Cannot inspect managed {package}: {error}"))?;
        if installed.as_deref() != Some(&version) {
            npm.install(&package, &version)
                .map_err(|error| format!("Cannot install managed {package}@{version}: {error}"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "managed_tests.rs"]
mod tests;
