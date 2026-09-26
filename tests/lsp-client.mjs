import { spawn } from 'node:child_process';
import { EventEmitter } from 'node:events';
import { pathToFileURL } from 'node:url';
import {
  createMessageConnection,
  StreamMessageReader,
  StreamMessageWriter,
} from 'vscode-jsonrpc/node.js';

export async function startServer(t, root, args) {
  const child = spawn(process.execPath, args, {
    cwd: root,
    env: {
      ...process.env,
      npm_config_offline: 'true',
      npm_config_registry: 'http://127.0.0.1:9',
    },
    stdio: ['pipe', 'pipe', 'pipe'],
  });
  let stderr = '';
  child.stderr.on('data', (data) => { stderr += data; });
  const connection = createMessageConnection(
    new StreamMessageReader(child.stdout),
    new StreamMessageWriter(child.stdin),
  );
  const messages = [];
  const diagnostics = new Map();
  const events = new EventEmitter();
  connection.onNotification('window/logMessage', ({ message }) => messages.push(message));
  connection.onNotification('textDocument/publishDiagnostics', (publication) => {
    diagnostics.set(publication.uri, publication.diagnostics);
    events.emit('diagnostics');
  });
  child.on('exit', () => connection.dispose());
  t.after(() => {
    connection.dispose();
    child.kill();
  });
  connection.listen();

  async function request(method, params) {
    let timer;
    try {
      return await Promise.race([
        connection.sendRequest(method, params),
        new Promise((_, reject) => {
          timer = setTimeout(() => reject(new Error(`${method} timed out\n${stderr}`)), 15000);
        }),
      ]);
    } finally {
      clearTimeout(timer);
    }
  }

  await request('initialize', {
    processId: process.pid,
    rootUri: pathToFileURL(root).href,
    capabilities: {},
    workspaceFolders: [{ name: 'test-monorepo', uri: pathToFileURL(root).href }],
  });
  await connection.sendNotification('initialized', {});

  return {
    messages,
    request,
    async open(path, languageId, text) {
      const uri = pathToFileURL(path).href;
      await connection.sendNotification('textDocument/didOpen', {
        textDocument: { uri, languageId, version: 1, text },
      });
      return uri;
    },
    async diagnostics(uri, predicate = () => true) {
      const matches = () => diagnostics.has(uri) && predicate(diagnostics.get(uri));
      if (!matches()) {
        await new Promise((resolve, reject) => {
          const cleanup = () => {
            clearTimeout(timer);
            events.off('diagnostics', check);
          };
          const check = () => {
            if (matches()) {
              cleanup();
              resolve();
            }
          };
          const timer = setTimeout(() => {
            cleanup();
            reject(new Error(`No matching diagnostics for ${uri}\n${JSON.stringify([...diagnostics])}\n${messages.join('\n')}\n${stderr}`));
          }, 15000);
          events.on('diagnostics', check);
        });
      }
      return diagnostics.get(uri);
    },
  };
}
