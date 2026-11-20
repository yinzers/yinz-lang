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
//
// Blue, in the theme's info colour so it follows the user's theme: red is for errors
// and yellow for lint, so these lesson hints get a colour of their own. The
// decoration API takes a ThemeColor only for border/outline/background/colour, not
// for `textDecoration`, so the underline is a dotted bottom border.
const promotableDecoration = vscode.window.createTextEditorDecorationType({
    border: '0 0 1px 0 dotted',
    borderColor: new vscode.ThemeColor('editorInfo.foreground'),
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

// Decorations belong to one TextEditor, and VS Code serves cached semantic tokens
// when a tab is shown again without asking the server, so the ranges are kept per
// document and re-applied whenever the visible editors change.
interface PromotableRanges {
    documentVersion: number;
    ranges: vscode.Range[];
}
const promotableRangesByDocument = new Map<string, PromotableRanges>();

const applyPromotableDecorations = (editor: vscode.TextEditor): void => {
    const cached = promotableRangesByDocument.get(editor.document.uri.toString());
    const isCurrent = cached !== undefined && cached.documentVersion === editor.document.version;
    editor.setDecorations(promotableDecoration, isCurrent ? cached.ranges : []);
};

const decoratePromotableTokens = (
    document: vscode.TextDocument,
    requestedVersion: number,
    tokens: vscode.SemanticTokens
): void => {
    // The document changed while the server answered: these ranges are stale, and
    // the edit already triggered a fresh request.
    if (document.version !== requestedVersion) return;
    const modifierNames =
        client?.initializeResult?.capabilities.semanticTokensProvider?.legend.tokenModifiers ?? [];
    const modifierIndex = modifierNames.indexOf(PROMOTABLE_MODIFIER);
    if (modifierIndex < 0) return;
    promotableRangesByDocument.set(document.uri.toString(), {
        documentVersion: requestedVersion,
        ranges: promotableRanges(tokens, 1 << modifierIndex),
    });
    for (const editor of vscode.window.visibleTextEditors) {
        if (editor.document === document) applyPromotableDecorations(editor);
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
                const requestedVersion = document.version;
                const tokens = await next(document, cancellation);
                if (tokens) decoratePromotableTokens(document, requestedVersion, tokens);
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

    context.subscriptions.push(
        vscode.window.onDidChangeVisibleTextEditors(editors => {
            editors.forEach(applyPromotableDecorations);
        }),
        vscode.workspace.onDidCloseTextDocument(document => {
            promotableRangesByDocument.delete(document.uri.toString());
        })
    );

    context.subscriptions.push(promotableDecoration, {
        dispose: () => { client?.stop(); },
    });
};

export const deactivate = (): Thenable<void> | undefined => client?.stop();
