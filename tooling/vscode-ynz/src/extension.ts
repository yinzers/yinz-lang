import * as vscode from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind,
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

// The server marks `array` / `let` tokens the compiler already promoted
// (`array` -> `fixed`, `let` -> `const`) with this semantic-token modifier.
const PROMOTABLE_MODIFIER = 'promotable';

// A dotted underline cannot be expressed with semanticTokenColorCustomizations
// (colour and font style only), so the extension draws it from the very ranges the
// client already receives as semantic tokens.
const promotableDecoration = vscode.window.createTextEditorDecorationType({
    textDecoration: 'underline dotted',
});

const promotableRanges = (tokens: vscode.SemanticTokens, modifierBit: number): vscode.Range[] => {
    const ranges: vscode.Range[] = [];
    let line = 0;
    let startCharacter = 0;
    for (let offset = 0; offset + 4 < tokens.data.length; offset += 5) {
        const deltaLine = tokens.data[offset];
        line += deltaLine;
        startCharacter = deltaLine === 0 ? startCharacter + tokens.data[offset + 1] : tokens.data[offset + 1];
        const length = tokens.data[offset + 2];
        const modifiers = tokens.data[offset + 4];
        if ((modifiers & modifierBit) !== 0) {
            ranges.push(new vscode.Range(line, startCharacter, line, startCharacter + length));
        }
    }
    return ranges;
};

const decoratePromotableTokens = (document: vscode.TextDocument, tokens: vscode.SemanticTokens): void => {
    const modifierNames =
        client?.initializeResult?.capabilities.semanticTokensProvider?.legend.tokenModifiers ?? [];
    const modifierIndex = modifierNames.indexOf(PROMOTABLE_MODIFIER);
    if (modifierIndex < 0) return;
    const ranges = promotableRanges(tokens, 1 << modifierIndex);
    for (const editor of vscode.window.visibleTextEditors) {
        if (editor.document === document) {
            editor.setDecorations(promotableDecoration, ranges);
        }
    }
};

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
            provideDocumentSemanticTokens: async (document, cancellation, next) => {
                const tokens = await next(document, cancellation);
                if (tokens) decoratePromotableTokens(document, tokens);
                return tokens;
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

    context.subscriptions.push(promotableDecoration, {
        dispose: () => { client?.stop(); },
    });
};

export const deactivate = (): Thenable<void> | undefined => client?.stop();
