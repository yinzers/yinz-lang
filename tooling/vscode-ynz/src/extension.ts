import * as vscode from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind,
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

export const activate = (context: vscode.ExtensionContext): void => {
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
        // Transform diagnostic messages for the Problems panel.
        // The LSP server uses "\n\nWHAT INSTEAD: ...\n\nWHY: ..." as a multi-section
        // format that renders well in hover popups but shows raw \n\n in the Problems
        // panel.  We collapse the sections to a single readable line for that surface.
        middleware: {
            handleDiagnostics: (uri, diagnostics, next) => {
                const transformed = diagnostics.map(d => {
                    if (!d.message.includes('\n\n')) return d;
                    // Keep only the first section (the WHAT) for the Problems panel.
                    const firstSection = d.message.split('\n\n')[0].trim();
                    return { ...d, message: firstSection };
                });
                next(uri, transformed);
            },
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
        dispose: () => { client?.stop(); },
    });
};

export const deactivate = (): Thenable<void> | undefined => client?.stop();
