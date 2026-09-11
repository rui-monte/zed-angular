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

No global or project-local language server installation is required. On its first start, the extension uses Zed's Node extension API to download matching releases of `@angular/language-server` and its runtime `@angular/language-service` package into extension-managed storage. A launcher shipped with the registered Angular language resources adds that storage to Angular's package probe locations before starting `ngserver`. The extension reuses those installations on subsequent starts and checks for updates once per extension session.

The server still probes the open worktree for the project's Angular and TypeScript packages, so application dependencies should be installed normally (for example with `npm install`). If the npm registry is temporarily unavailable, an already downloaded server remains usable.

To opt out of the managed server, set `angular_language_server_path` to a local installation as described below.

## Version Management

The extension manages the latest `@angular/language-server` package and uses the `typescript` package available in your project.

The major version of `@angular/language-server` must match the Angular major version used by your project. TypeScript must be **5.0 or later**, and **6.0.3 is the latest supported version** — newer releases are untested and may fail to load. Mismatches typically surface as a `Failed to resolve 'typescript/lib/tsserverlibrary'` error in the language server logs.

If your project would otherwise pull in a newer TypeScript, pin it:

```json
{
  "devDependencies": {
    "typescript": "~6.0.3"
  }
}
```

Refer to [Angular Version Compatibility](https://angular.dev/reference/versions#unsupported-angular-versions) for details.

## Configuration

All options are set under `lsp.angular.initialization_options` in your Zed `settings.json` (or a project-local `.zed/settings.json`):

| Option                         | Type     | Default                                  | Description                                                                        |
| ------------------------------ | -------- | ---------------------------------------- | ---------------------------------------------------------------------------------- |
| `angular_language_server_path` | `string` | extension-managed installation           | Optional location of a custom `@angular/language-server` package directory.        |
| `max_ts_server_memory`         | `number` | unset (node default, ~4 GB)              | Heap limit in MB, passed to node as `--max-old-space-size`.                         |

Both can be combined — this is the typical monorepo setup, where the app lives in a subfolder *and* the project is large enough to exhaust node's default heap:

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

TypeScript and Angular are then probed in the worktree root, its `node_modules`, and the ancestors of the resolved package directory — so a server under `client/` still resolves `client/node_modules/typescript` correctly.

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

Start at `8192` and increase only if crashes persist; the value is a ceiling, not a reservation, so node allocates lazily. Setting it above the machine's available RAM will trade crashes for swapping. For the managed executable the extension appends the flag to `NODE_OPTIONS`; for a custom server path it emits the flag before the server script path. It is omitted entirely when the option is unset.

## Installation Instructions

To install this extension locally:

1. Clone this repository.
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
