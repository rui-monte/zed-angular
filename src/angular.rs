use serde::Deserialize;
use zed::lsp::{Completion, CompletionKind};
use zed::settings::LspSettings;
use zed::{CodeLabelSpan, LanguageServerInstallationStatus};
use zed_extension_api::{self as zed, serde_json, Result};

const SERVER_PACKAGE: &str = "@angular/language-server";
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
    /// installed through the extension API live in the extension's working
    /// directory, so this deliberately returns a relative path rather than a
    /// path below the user's worktree.
    fn managed_server_dir(&mut self, language_server_id: &zed::LanguageServerId) -> Result<String> {
        if self.managed_server_version.is_some() {
            return Ok(MANAGED_SERVER_DIR.to_string());
        }

        zed::set_language_server_installation_status(
            language_server_id,
            &LanguageServerInstallationStatus::CheckingForUpdate,
        );

        let installed = match zed::npm_package_installed_version(SERVER_PACKAGE) {
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
            Err(_error) if installed.is_some() => {
                self.managed_server_version = installed;
                zed::set_language_server_installation_status(
                    language_server_id,
                    &LanguageServerInstallationStatus::None,
                );
                return Ok(MANAGED_SERVER_DIR.to_string());
            }
            Err(error) => {
                zed::set_language_server_installation_status(
                    language_server_id,
                    &LanguageServerInstallationStatus::Failed(error.clone()),
                );
                return Err(error);
            }
        };

        if installed.as_deref() != Some(latest.as_str()) {
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

        self.managed_server_version = Some(latest);
        zed::set_language_server_installation_status(
            language_server_id,
            &LanguageServerInstallationStatus::None,
        );
        Ok(MANAGED_SERVER_DIR.to_string())
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
            None => self.managed_server_dir(language_server_id)?,
        };
        let probes = Self::probe_locations(&root, &server_dir);

        let mut args = Vec::new();

        // Node flags must come before the script path.
        if let Some(mb) = settings.max_ts_server_memory {
            args.push(format!("--max-old-space-size={mb}"));
        }

        args.push(format!("{server_dir}/index.js"));
        args.push("--stdio".into());
        args.push("--tsProbeLocations".into());
        args.push(probes.clone());
        args.push("--ngProbeLocations".into());
        args.push(probes);
        args.push("--logToConsole".into());
        args.push("--logVerbosity".into());
        args.push("normal".into());

        Ok(zed::Command {
            command: zed::node_binary_path()?,
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

zed::register_extension!(AngularExtension);
