# Testing inside Zed

The Rust unit tests and Node LSP tests exercise extension logic and the real server. This manual check also exercises Zed's WebAssembly host, npm API, language registration, and editor UI. Follow Zed's [dev-extension workflow](https://zed.dev/docs/extensions/developing-extensions#developing-an-extension-locally).

## Setup

1. Install Rust via rustup and make `rustc` and `cargo` available on Zed's PATH. Zed compiles the extension for `wasm32-wasip2` and downloads the WASI SDK to build its grammar.
2. Start Zed with an empty, disposable `--user-data-dir`, then use **Install Dev Extension** to select this checkout. Confirm that Angular appears as a dev extension.
3. Create a temporary worktree with `apps/angular18`, `apps/angular21`, and `apps/angular22`. Give each directory its own `package.json` and install only its application dependency: `@angular/core@18.2.14`, `@angular/core@21.2.24`, or `@angular/core@22.2.0`, respectively. Do not install a language server in those projects.
4. Put the following files in each subproject and open the common worktree root in Zed.

`tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "experimentalDecorators": true,
    "strict": true,
    "skipLibCheck": true,
    "noEmit": true
  },
  "angularCompilerOptions": { "strictTemplates": true },
  "files": ["app.ts"]
}
```

`app.ts`:

```ts
import { Component, NgModule } from '@angular/core';

@Component({ selector: 'test-app', templateUrl: './app.html' })
export class AppComponent {
  title = 'Hello';
}

@NgModule({ declarations: [AppComponent] })
export class AppModule {}
```

`app.html`:

```html
<h1>{{ title }}</h1>
<p>{{ missing }}</p>
```

Use this worktree's `.zed/settings.json` to isolate the Angular server:

```json
{
  "languages": {
    "TypeScript": { "language_servers": ["angular"] },
    "HTML": { "language_servers": ["angular"] },
    "Angular": { "language_servers": ["angular"] }
  }
}
```

## Checks

| Action | Expected result |
| --- | --- |
| Open each `app.ts`, wait for diagnostics, then open its template | The server starts without a project-local language-server package. |
| Run `dev: open language server logs` | TypeScript 6.0.3 and Angular language service 22.2.0 load from the chosen profile's extension storage. |
| Hover `title` in all three templates | `(property) AppComponent.title: string`. |
| Inspect `missing` | Angular reports that the property does not exist on `AppComponent`. |
| Replace `missing` with `tit`, then accept the `title` completion | Completion inserts `title`; the template diagnostic clears. |
| Inspect `@NgModule` declarations | Angular 18 permits the declaration; Angular 21 and 22 report that the component is standalone and cannot be declared in an NgModule. |
| Rename one template to `app.component.html` and update `templateUrl` | Zed selects the Angular grammar, and template hover and diagnostics still work. |
| Quit Zed and reopen the same profile with `npm_config_offline=true npm_config_registry=http://127.0.0.1:9` | Language features return, with unchanged managed package versions and modification times. Verify the Zed and server processes inherited those variables. This restricts npm, not all machine networking. |
| Set `lsp.angular.initialization_options.angular_language_server_path` to a separately installed server, set `max_ts_server_memory`, and restart the language server | Logs show the selected installation; its process uses the memory flag before the script path, and template hover still works. |

Restore the temporary worktree's default managed-server settings and relaunch without the offline environment after testing.

## Recorded run

On 2026-09-26, these checks passed in Zed **1.21.0** on macOS arm64 using an isolated profile and real Angular 18.2.14, 21.2.24, and 22.2.0 packages. Completion and diagnostic clearing were checked in Angular 18; hover, template diagnostics, and standalone behavior were checked across all three versions. The Angular grammar and npm-offline restart were checked with Angular 22. The custom-server check used a separate installation of the pinned stack and a 4096 MB heap limit.

The first dev install exposed a local setup problem: Zed's PATH did not contain `rustc`. Relaunching with the installed Rust toolchain on PATH allowed Zed to compile and install the extension. CI and the development commands now use Zed's current `wasm32-wasip2` target. These are recorded manual results; CI does not drive the Zed UI.
