import * as path from 'path';
import * as vscode from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind,
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext): void {
    const serverPath = vscode.workspace
        .getConfiguration('yinz')
        .get<string>('server.path', 'ynz-lsp');

    const serverOptions: ServerOptions = {
        command: serverPath,
        args: [],
        transport: TransportKind.stdio,
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'ynz' }],
        synchronize: {
            fileEvents: vscode.workspace.createFileSystemWatcher('**/*.ynz'),
        },
    };

    client = new LanguageClient(
        'yinz',
        'Yinz Language Server',
        serverOptions,
        clientOptions
    );

    client.start().catch((err: Error) => {
        vscode.window.showErrorMessage(
            `Failed to start ynz-lsp: ${err.message}. ` +
            `Make sure 'ynz-lsp' is on your PATH or set yinz.server.path in settings.`
        );
    });

    context.subscriptions.push({
        dispose: () => client?.stop(),
    });
}

export function deactivate(): Thenable<void> | undefined {
    return client?.stop();
}
