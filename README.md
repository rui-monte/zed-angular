# Zed Angular Extension

## Overview

**Note: This project is currently a work in progress. Expect potential bugs or issues.**

This extension integrates the Angular Language Service into Zed. It uses the same options that Angular applies during compilation. To ensure the most accurate information, enable the `strictTemplates` option in the `tsconfig.json` of the angular project as shown in below:

```json
"angularCompilerOptions": {
  "strictTemplates": true
}
```

## Automatic Language Server Installation

No global or project-local language server installation is required. The extension uses Zed's Node extension API to install the exact versions in [`managed-server/package.json`](managed-server/package.json) into Zed's extension storage:

| Package | Pinned version |
| --- | --- |
| `@angular/language-server` | `22.2.0` |
| `@angular/language-service` | `22.2.0` |
| `typescript` | `6.0.3` |

Startup checks the installed versions locally. Matching packages are reused without npm registry access, including across Zed sessions. Missing packages or versions left by an older extension release are installed at the pinned versions. The extension never checks npm `latest`; changing the managed stack requires an extension update.

The first installation requires network access. If an installation is incomplete or has different versions, startup reports the package that could not be installed instead of running a mixed stack. Reconnect and restart the language server to retry. Once all three pinned packages are present, startup works offline.

Install your application's dependencies normally (for example with `npm install`). The language service still needs the project's Angular packages and `tsconfig.json`. The managed stack requires Node `^22.22.3`, `^24.15.0`, or `>=26.0.0`, obtained through Zed's Node API.

To opt out of the managed server, set `angular_language_server_path` to a local installation as described below.

## TypeScript and Angular Compatibility

In managed mode, both `--tsProbeLocations` and `--ngProbeLocations` point only to the extension's storage. The server uses its pinned TypeScript and Angular language-service packages even when the project contains different versions. Your application's TypeScript build dependency is independent and does not need to match the editor's version.

The managed Angular language service [detects `@angular/core` relative to each project's `tsconfig.json`](https://github.com/angular/angular/commit/8a7cbd46685874f4500c52629d09c5f7fd309080). It can therefore use different Angular compatibility settings for nested projects in the same worktree. The extension does not force a single Angular core version for the whole workspace.

Runtime tests exercise Angular **18.2.14**, **21.2.24**, and **22.2.0** together, checking template hovers, diagnostics, and the different standalone-component defaults before and after Angular 19. This is the verified compatibility set for this stack, not a guarantee for every Angular version or compiler option. Other versions, particularly older releases or projects newer than the managed service, may need a custom server.

Use `angular_language_server_path` for a project-local or manually pinned server when needed. Fallback to a local server is explicit; the extension does not switch implementations based on the project's Angular version.

## Configuration

All options are set under `lsp.angular.initialization_options` in your Zed `settings.json` (or a project-local `.zed/settings.json`):

| Option                         | Type     | Default                                  | Description                                                                        |
| ------------------------------ | -------- | ---------------------------------------- | ---------------------------------------------------------------------------------- |
| `angular_language_server_path` | `string` | extension-managed installation           | Optional location of a custom `@angular/language-server` package directory.        |
| `max_ts_server_memory`         | `number` | unset (node default, ~4 GB)              | Heap limit in MB, passed to node as `--max-old-space-size`.                         |

Both can be combined when a monorepo needs a custom server and a larger heap:

```json
{
  "lsp": {
    "angular": {
      "initialization_options": {
        "angular_language_server_path": "client/node_modules/@angular/language-server",
        "max_ts_server_memory": 8192
      }
    }
  }
}
```

Both options are optional and independent; omit either one to keep its default.

### Custom Server Path

Set `angular_language_server_path` to opt out of automatic installation—for example, to pin the server to the Angular major version installed in a project or to use a monorepo package.

The value must be the **package directory**, not the `index.js` file inside it (a trailing `/index.js` is tolerated and stripped). Accepted forms:

| Form              | Example                                                        | Resolved against                    |
| ----------------- | -------------------------------------------------------------- | ----------------------------------- |
| Worktree-relative | `client/node_modules/@angular/language-server`                 | The root of the open worktree       |
| Absolute          | `/Users/me/repo/client/node_modules/@angular/language-server`  | Used as-is                          |
| Home-relative     | `~/.npm-global/lib/node_modules/@angular/language-server`      | `$HOME` from your shell environment |

The path is **not validated** by the extension. Zed extensions run sandboxed and can only inspect files present in the project's file index, which excludes gitignored trees such as `node_modules`, so any existence check would report false negatives. If the path is wrong, Node reports a `MODULE_NOT_FOUND` error in the language server logs instead.

Custom mode skips all managed-package checks and installations. TypeScript and the Angular language service are probed in the worktree root, its `node_modules`, and the ancestors of the resolved package directory, in that order. A server under `client/` can therefore resolve `client/node_modules/typescript`. You are responsible for selecting versions compatible with the custom server; the managed TypeScript pin does not apply in this mode.

### Memory

In large workspaces (e.g. monorepos), the language server can exceed node's default heap limit (~4 GB) and crash repeatedly. If the server keeps restarting or stops responding after a few minutes, raise the limit with `max_ts_server_memory` (in MB, passed to node as `--max-old-space-size`):

```json
{
  "lsp": {
    "angular": {
      "initialization_options": {
        "max_ts_server_memory": 8192
      }
    }
  }
}
```

Start at `8192` and increase only if crashes persist; the value is a ceiling, not a reservation, so node allocates lazily. Setting it above the machine's available RAM will trade crashes for swapping. The extension emits the flag before the server script path so Node interprets it. It is omitted entirely when the option is unset.

## Installation Instructions

To install this extension locally:

1. Clone this repository and [install Rust via rustup](https://zed.dev/docs/extensions/developing-extensions#developing-an-extension-locally). Zed must be able to find `rustc` and `cargo` on its PATH.
2. Open the Zed editor and navigate to the Extensions window.
3. Click on "Install Dev Extension."
4. Select the cloned repository location and complete the installation.
5. Add a language server list definition to the HTML and TypeScript language settings. In `settings.json`, add the following _(ellipsis is a valid value in settings, use it as shown)_:

```json
{
  "languages": {
    "TypeScript": {
      "language_servers": ["angular", "..."]
    },
    "HTML": {
      "language_servers": ["angular", "..."]
    }
  }
}
```

If the published version of the extension is already installed, Zed uninstalls it before installing the dev extension. After changing the source, run `zed: rebuild dev extension` from the command palette — the extension is compiled to WebAssembly at install/rebuild time, so edits are not picked up until you do.

## Development Checks

The Rust tests cover installation, offline reuse, retries, custom paths, and launch arguments:

```sh
cargo fmt -- --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
rustup target add wasm32-wasip2
cargo build --locked --target wasm32-wasip2
```

To run the actual language server against the Angular fixtures, use a Node version supported by the managed stack and install the locked test dependencies:

```sh
npm ci --prefix managed-server --ignore-scripts --no-audit --no-fund
npm ci --prefix tests --ignore-scripts --no-audit --no-fund
cargo test --locked managed_server_runtime -- --ignored --nocapture
```

This test uses the extension's real launch arguments, starts the server with npm offline, and opens temporary projects containing real Angular packages without a project-local language server. It also places conflicting TypeScript and language-service packages at the monorepo root to verify managed probe isolation. These checks run in CI; they do not automate Zed's UI.

For checks inside the editor, follow the [Zed smoke-test procedure](tests/zed-smoke-test.md). It covers dev-extension installation, diagnostics, hover, completion, reuse across editor sessions, and the custom-server override.

When updating the managed stack, change the exact versions in `managed-server/package.json`, regenerate its lockfile, update the version table above, and rerun both sets of tests. The extension embeds that manifest at compile time.
