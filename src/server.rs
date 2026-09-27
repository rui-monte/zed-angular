pub struct Server {
    directory: String,
    probes: String,
}

impl Server {
    pub fn managed(extension_directory: &str) -> Self {
        let directory = normalize(extension_directory);
        Self {
            directory: format!("{directory}/node_modules/@angular/language-server"),
            // Project packages must not replace any part of the pinned stack.
            // Angular itself resolves each project's @angular/core from its tsconfig.
            probes: directory,
        }
    }

    pub fn custom(root: &str, requested: &str, env: &[(String, String)]) -> Self {
        let directory = resolve_custom_directory(root, requested, env);
        let probes = custom_probe_locations(root, &directory);
        Self { directory, probes }
    }

    pub fn arguments(&self, max_memory: Option<u32>) -> Vec<String> {
        let mut args = Vec::new();
        // Node options must precede the script path.
        if let Some(mb) = max_memory {
            args.push(format!("--max-old-space-size={mb}"));
        }
        args.extend([
            format!("{}/index.js", self.directory),
            "--stdio".into(),
            "--tsProbeLocations".into(),
            self.probes.clone(),
            "--ngProbeLocations".into(),
            self.probes.clone(),
            "--logToConsole".into(),
            "--logVerbosity".into(),
            "normal".into(),
        ]);
        args
    }
}

fn normalize(path: &str) -> String {
    path.trim().replace('\\', "/").trim_end_matches('/').into()
}

fn resolve_custom_directory(root: &str, requested: &str, env: &[(String, String)]) -> String {
    let requested = normalize(requested);
    let requested = requested.strip_suffix("/index.js").unwrap_or(&requested);
    let expanded = match requested.strip_prefix("~/") {
        Some(rest) => env
            .iter()
            .find(|(key, value)| (key == "HOME" || key == "USERPROFILE") && !value.is_empty())
            .map(|(_, home)| format!("{}/{rest}", normalize(home)))
            .unwrap_or_else(|| requested.to_string()),
        None => requested.to_string(),
    };

    let is_drive_absolute = expanded.as_bytes().get(1..3) == Some(b":/");
    if expanded.starts_with('/') || is_drive_absolute {
        expanded
    } else {
        // Worktree snapshots omit gitignored node_modules. Let Node validate
        // custom paths against the real filesystem instead.
        format!("{}/{expanded}", normalize(root))
    }
}

fn custom_probe_locations(root: &str, server_directory: &str) -> String {
    let root = normalize(root);
    let mut paths = vec![root.clone(), format!("{root}/node_modules")];

    // Preserve project-first probing and include a nested server's package root.
    // For client/node_modules/@angular/language-server this reaches client/.
    let mut current = server_directory;
    for _ in 0..3 {
        let Some((parent, _)) = current.rsplit_once('/') else {
            break;
        };
        if parent.is_empty() {
            break;
        }
        if !paths.iter().any(|path| path == parent) {
            paths.push(parent.to_string());
        }
        current = parent;
    }
    paths.join(",")
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
