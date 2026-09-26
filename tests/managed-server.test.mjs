import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import test from 'node:test';

import { startServer } from './lsp-client.mjs';

// Cargo supplies the actual launch arguments produced by the extension.
const args = JSON.parse(process.env.ZED_ANGULAR_SERVER_ARGS);
const manifest = JSON.parse(await readFile(new URL('../managed-server/package.json', import.meta.url)));
const template = '<h1>{{ title }}</h1><p>{{ missing }}</p>';
const component = `
import { Component, NgModule } from '@angular/core';

@Component({ selector: 'test-app', templateUrl: './app.html' })
export class AppComponent { title = 'Hello'; }

@NgModule({ declarations: [AppComponent] })
export class AppModule {}
`;

async function linkCore(root, major) {
  const require = createRequire(new URL(`./fixtures/angular${major}/package.json`, import.meta.url));
  const packageDirectory = dirname(require.resolve('@angular/core/package.json'));
  await mkdir(join(root, 'node_modules/@angular'), { recursive: true });
  await symlink(packageDirectory, join(root, 'node_modules/@angular/core'), 'junction');
}

async function createProject(root, major) {
  const project = join(root, 'apps', `angular${major}`);
  await linkCore(project, major);
  await writeFile(join(project, 'tsconfig.json'), JSON.stringify({
    compilerOptions: {
      target: 'ES2022',
      module: 'ESNext',
      moduleResolution: 'bundler',
      experimentalDecorators: true,
      strict: true,
      skipLibCheck: true,
      noEmit: true,
    },
    angularCompilerOptions: { strictTemplates: true },
    files: ['app.ts'],
  }));
  await writeFile(join(project, 'app.ts'), component);
  await writeFile(join(project, 'app.html'), template);
  assert.equal(existsSync(join(project, 'node_modules/@angular/language-server')), false);
  return project;
}

async function temporaryWorkspace(t) {
  const root = await mkdtemp(join(tmpdir(), 'zed-angular-monorepo-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  return root;
}

async function shadowPackage(root, name) {
  const directory = join(root, 'node_modules', name);
  await mkdir(directory, { recursive: true });
  await writeFile(join(directory, 'package.json'), JSON.stringify({ name, version: '99.0.0', main: 'index.js' }));
  await writeFile(join(directory, 'index.js'), "throw new Error('Project package replaced the managed stack');");
  if (name === 'typescript') {
    await mkdir(join(directory, 'lib'));
    await writeFile(join(directory, 'lib/tsserverlibrary.js'), "throw new Error('Project TypeScript was loaded');");
  }
}

test('the managed stack detects Angular per project in a mixed-version monorepo', { timeout: 60000 }, async (t) => {
  const root = await temporaryWorkspace(t);
  // The root version differs from the nested Angular 18 and 21 projects.
  await linkCore(root, 22);
  await shadowPackage(root, 'typescript');
  await shadowPackage(root, '@angular/language-service');
  const server = await startServer(t, root, args);

  for (const major of [22, 18, 21]) {
    await t.test(`Angular ${major}: template hover, diagnostics, and standalone defaults`, async () => {
      const project = await createProject(root, major);
      const componentUri = await server.open(join(project, 'app.ts'), 'typescript', component);
      // Opening an external template replaces Angular's pending diagnostic
      // batch. Await the component publication before opening its template.
      const isStandaloneError = (item) => /standalone.*cannot be declared/i.test(item.message);
      const diagnostics = await server.diagnostics(componentUri,
        (items) => major < 19 || items.some(isStandaloneError));
      assert.equal(diagnostics.some(isStandaloneError), major >= 19,
        'Standalone defaults must come from the project Angular version, not the managed service or workspace root');

      const templateUri = await server.open(join(project, 'app.html'), 'html', template);
      await server.diagnostics(templateUri, (items) => items.some((item) => /missing/.test(item.message)));

      const hover = await server.request('textDocument/hover', {
        textDocument: { uri: templateUri },
        position: { line: 0, character: template.indexOf('title') + 1 },
      });
      assert.match(JSON.stringify(hover), /title.*string/);
    });
  }

  assert.ok(server.messages.some((message) => message.includes(`is version ${manifest.dependencies.typescript}.`)));
  assert.ok(server.messages.some((message) => message.includes(`@angular/language-service v${manifest.dependencies['@angular/language-service']}`)));
});

test('a fresh server process reuses the installed stack with npm offline', { timeout: 30000 }, async (t) => {
  const root = await temporaryWorkspace(t);
  const project = await createProject(root, 18);
  const server = await startServer(t, root, args);
  await server.open(join(project, 'app.ts'), 'typescript', component);
  const uri = await server.open(join(project, 'app.html'), 'html', template);
  const diagnostics = await server.diagnostics(uri, (items) => items.some((item) => /missing/.test(item.message)));
  assert.ok(diagnostics.some((item) => /missing/.test(item.message)));
});
