mod managed;
mod server;

use serde::Deserialize;
use zed::lsp::{Completion, CompletionKind};
use zed::settings::LspSettings;
use zed::CodeLabelSpan;
use zed_extension_api::{self as zed, serde_json, Result};

use server::Server;

#[derive(Deserialize, Default)]
struct UserSettings {
    /// Maximum Node heap size in MB.
    max_ts_server_memory: Option<u32>,
    /// Optional package directory; bypasses the managed server entirely.
    angular_language_server_path: Option<String>,
}

struct AngularExtension;

impl zed::Extension for AngularExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let settings: UserSettings =
            LspSettings::for_worktree(language_server_id.as_ref(), worktree)
                .ok()
                .and_then(|settings| settings.initialization_options)
                .map(serde_json::from_value)
                .transpose()
                .map_err(|error| {
                    format!("Failed to parse `lsp.angular.initialization_options`: {error}")
                })?
                .unwrap_or_default();

        let env = worktree.shell_env();
        let server = match settings
            .angular_language_server_path
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())
        {
            Some(path) => Server::custom(&worktree.root_path(), path, &env),
            None => managed::server(language_server_id)?,
        };

        Ok(zed::Command {
            command: zed::node_binary_path()?,
            args: server.arguments(settings.max_ts_server_memory),
            env,
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
