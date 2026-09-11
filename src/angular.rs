use serde::Deserialize;
use zed::lsp::{Completion, CompletionKind};
use zed::settings::LspSettings;
use zed::{CodeLabelSpan, LanguageServerInstallationStatus, Os};
use zed_extension_api::{self as zed, serde_json, Result};

const SERVER_PACKAGE: &str = "@angular/language-server";
const LANGUAGE_SERVICE_PACKAGE: &str = "@angular/language-service";
const MANAGED_SERVER_DIR: &str = "node_modules/@angular/language-server";

#[derive(Deserialize, Default)]
struct UserSettings {
    /// Maximum heap size (in MB) for the language server process, passed to
    /// node as `--max-old-space-size`.
    max_ts_server_memory: Option<u32>,
    /// Override the location of the `@angular/language-server` package.
    /// Worktree-relative, absolute, or `~`-prefixed. When omitted, the
    /// extension installs and maintains its own copy.
    angular_language_server_path: Option<String>,
}

struct AngularExtension {
    managed_server_version: Option<String>,
}

impl AngularExtension {
    fn env<'a>(env: &'a [(String, String)], name: &str) -> Option<&'a str> {
        env.iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
            .filter(|value| !value.is_empty())
    }

    /// Return the absolute directory used by Zed's npm extension API.
    ///
    /// Language-server commands run with the project as their working
    /// directory. Consequently, relative script and probe paths point into
    /// the project, even though npm packages are installed in Zed's extension
    /// work directory.
    fn managed_extension_dir(worktree: &zed::Worktree) -> Result<String> {
        let env = worktree.shell_env();
        let (os, _) = zed::current_platform();

        Self::managed_extension_dir_for(os, &env)
    }

    fn managed_extension_dir_for(os: Os, env: &[(String, String)]) -> Result<String> {
        let data_dir = match os {
            Os::Mac => {
                let home = Self::env(&env, "HOME").ok_or_else(|| {
                    "Cannot locate Zed extension storage: HOME is not set".to_string()
                })?;
                format!(
                    "{}/Library/Application Support/Zed",
                    home.trim_end_matches('/')
                )
            }
            Os::Linux => match Self::env(&env, "XDG_DATA_HOME") {
                Some(data_home) => format!("{}/zed", data_home.trim_end_matches('/')),
                None => {
                    let home = Self::env(&env, "HOME").ok_or_else(|| {
                        "Cannot locate Zed extension storage: HOME and XDG_DATA_HOME are not set"
                            .to_string()
                    })?;
                    format!("{}/.local/share/zed", home.trim_end_matches('/'))
                }
            },
            Os::Windows => {
                let local_app_data = Self::env(&env, "LOCALAPPDATA").ok_or_else(|| {
                    "Cannot locate Zed extension storage: LOCALAPPDATA is not set".to_string()
                })?;
                format!("{}/Zed", local_app_data.trim_end_matches(['/', '\\'])).replace('\\', "/")
            }
        };

        Ok(format!("{data_dir}/extensions/work/angular"))
    }

    /// Trim whitespace, a trailing `/index.js`, and trailing slashes so the
    /// value always denotes the package *directory*.
    fn normalize(path: &str) -> String {
        let p = path.trim().replace('\\', "/");
        let p = p.strip_suffix("/index.js").unwrap_or(&p);
        p.trim_end_matches('/').to_string()
    }

    fn expand_home(worktree: &zed::Worktree, path: &str) -> String {
        let rest = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\"));

        match rest {
            Some(rest) => {
                let env = worktree.shell_env();
                let home = env
                    .iter()
                    .find(|(k, _)| k == "HOME" || k == "USERPROFILE")
                    .map(|(_, v)| v.as_str());

                match home {
                    Some(home) => {
                        let home = home.trim_end_matches(['/', '\\']);
                        format!("{home}/{rest}")
                    }
                    None => path.to_string(),
                }
            }
            None => path.to_string(),
        }
    }

    /// Resolve the language server package directory to an absolute path.
    ///
    /// Deliberately does not verify existence: `Worktree::read_text_file` reads
    /// from Zed's worktree snapshot, which excludes gitignored trees such as
    /// `node_modules` and unexpanded symlinked directories, so any check here
    /// yields false negatives. node resolves the path against the real
    /// filesystem and reports `MODULE_NOT_FOUND` if it is wrong.
    fn resolve_custom_server_dir(worktree: &zed::Worktree, override_path: &str) -> String {
        let root = worktree.root_path();
        let root = root.trim_end_matches('/');

        let requested = Self::normalize(&Self::expand_home(worktree, override_path));

        let is_drive_abs = requested.len() > 2
            && requested.as_bytes()[1] == b':'
            && requested.as_bytes()[2] == b'/';
        if requested.starts_with('/') || is_drive_abs {
            requested
        } else {
            format!("{root}/{requested}")
        }
    }

    /// Install and update the extension-managed language server. NPM packages
    /// installed through the extension API live in Zed's extension working
    /// directory. Return an absolute path because the server process itself
    /// runs with the user's project as its working directory.
    fn managed_server_dir(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<String> {
        let server_dir = format!(
            "{}/{}",
            Self::managed_extension_dir(worktree)?,
            MANAGED_SERVER_DIR
        );

        if self.managed_server_version.is_some() {
            return Ok(server_dir);
        }

        zed::set_language_server_installation_status(
            language_server_id,
            &LanguageServerInstallationStatus::CheckingForUpdate,
        );

        let installed_server = match zed::npm_package_installed_version(SERVER_PACKAGE) {
            Ok(version) => version,
            Err(error) => {
                zed::set_language_server_installation_status(
                    language_server_id,
                    &LanguageServerInstallationStatus::Failed(error.clone()),
                );
                return Err(error);
            }
        };
        let installed_service = match zed::npm_package_installed_version(LANGUAGE_SERVICE_PACKAGE) {
            Ok(version) => version,
            Err(error) => {
                zed::set_language_server_installation_status(
                    language_server_id,
                    &LanguageServerInstallationStatus::Failed(error.clone()),
                );
                return Err(error);
            }
        };
        let latest = match zed::npm_package_latest_version(SERVER_PACKAGE) {
            Ok(version) => version,
            Err(_error) if installed_server.is_some() && installed_service.is_some() => {
                self.managed_server_version = installed_server;
                zed::set_language_server_installation_status(
                    language_server_id,
                    &LanguageServerInstallationStatus::None,
                );
                return Ok(server_dir);
            }
            Err(error) => {
                zed::set_language_server_installation_status(
                    language_server_id,
                    &LanguageServerInstallationStatus::Failed(error.clone()),
                );
                return Err(error);
            }
        };

        if installed_server.as_deref() != Some(latest.as_str()) {
            zed::set_language_server_installation_status(
                language_server_id,
                &LanguageServerInstallationStatus::Downloading,
            );
            if let Err(error) = zed::npm_install_package(SERVER_PACKAGE, &latest) {
                zed::set_language_server_installation_status(
                    language_server_id,
                    &LanguageServerInstallationStatus::Failed(error.clone()),
                );
                return Err(error);
            }
        }

        // `@angular/language-server` resolves this package from its probe
        // locations at runtime but does not install it itself. Keep both
        // Angular packages on the same release so a managed installation is
        // actually self-contained.
        if installed_service.as_deref() != Some(latest.as_str()) {
            zed::set_language_server_installation_status(
                language_server_id,
                &LanguageServerInstallationStatus::Downloading,
            );
            if let Err(error) = zed::npm_install_package(LANGUAGE_SERVICE_PACKAGE, &latest) {
                zed::set_language_server_installation_status(
                    language_server_id,
                    &LanguageServerInstallationStatus::Failed(error.clone()),
                );
                return Err(error);
            }
        }

        self.managed_server_version = Some(latest);
        zed::set_language_server_installation_status(
            language_server_id,
            &LanguageServerInstallationStatus::None,
        );
        Ok(server_dir)
    }

    /// Probe roots: the worktree root, its `node_modules`, and each ancestor of
    /// the resolved package directory (covers layouts such as
    /// `client/node_modules/...`).
    fn probe_locations(root: &str, server_dir: &str) -> String {
        let root = root.trim_end_matches('/');
        let mut paths = vec![root.to_string(), format!("{root}/node_modules")];

        let mut current = server_dir;
        for _ in 0..3 {
            match current.rsplit_once('/') {
                Some((head, _)) if !head.is_empty() => {
                    paths.push(head.to_string());
                    current = head;
                }
                _ => break,
            }
        }

        let mut unique = Vec::with_capacity(paths.len());
        for p in paths {
            if !unique.contains(&p) {
                unique.push(p);
            }
        }
        unique.join(",")
    }
}

impl zed::Extension for AngularExtension {
    fn new() -> Self {
        Self {
            managed_server_version: None,
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let settings: UserSettings =
            LspSettings::for_worktree(language_server_id.as_ref(), worktree)
                .ok()
                .and_then(|s| s.initialization_options)
                .map(serde_json::from_value)
                .transpose()
                .map_err(|e| format!("Failed to parse `lsp.angular.initialization_options`: {e}"))?
                .unwrap_or_default();

        let root = worktree.root_path();
        let server_dir = match settings
            .angular_language_server_path
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())
        {
            Some(path) => Self::resolve_custom_server_dir(worktree, path),
            None => self.managed_server_dir(language_server_id, worktree)?,
        };
        let probes = Self::probe_locations(&root, &server_dir);

        let mut args = Vec::new();

        // Node flags must come before the script path.
        if let Some(mb) = settings.max_ts_server_memory {
            args.push(format!("--max-old-space-size={mb}"));
        }
        args.push(format!("{server_dir}/index.js"));
        let command = zed::node_binary_path()?;

        args.push("--stdio".into());
        args.push("--tsProbeLocations".into());
        args.push(probes.clone());
        args.push("--ngProbeLocations".into());
        args.push(probes);
        args.push("--logToConsole".into());
        args.push("--logVerbosity".into());
        args.push("normal".into());

        Ok(zed::Command {
            command,
            args,
            env: worktree.shell_env(),
        })
    }

    fn label_for_completion(
        &self,
        _language_server_id: &zed::LanguageServerId,
        completion: Completion,
    ) -> Option<zed::CodeLabel> {
        let highlight_name = match completion.kind? {
            CompletionKind::Class | CompletionKind::Interface => "type",
            CompletionKind::Constructor => "constructor",
            CompletionKind::Constant => "constant",
            CompletionKind::Function | CompletionKind::Method => "function",
            CompletionKind::Property | CompletionKind::Field => "property",
            CompletionKind::Variable => "variable",
            CompletionKind::Keyword => "keyword",
            CompletionKind::Enum => "enum",
            CompletionKind::Module => "module",
            _ => return None,
        };

        let len = completion.label.len();
        let name_span = CodeLabelSpan::literal(completion.label, Some(highlight_name.to_string()));

        let spans = match completion.detail {
            Some(detail) => vec![
                name_span,
                CodeLabelSpan::literal(" ", None),
                CodeLabelSpan::literal(detail, Some("comment".to_string())),
            ],
            None => vec![name_span],
        };

        Some(zed::CodeLabel {
            code: Default::default(),
            spans,
            filter_range: (0..len).into(),
        })
    }
}

#[cfg(test)]
mod tests {
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
            AngularExtension::managed_extension_dir_for(
                Os::Mac,
                &env(&[("HOME", "/Users/ruimonte")]),
            ),
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
}

zed::register_extension!(AngularExtension);
