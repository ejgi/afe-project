"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const vscode = __importStar(require("vscode"));
const path = __importStar(require("path"));
const CSVEditorProvider_1 = require("./CSVEditorProvider");
function activate(context) {
    console.log('Big Data Explorer is now active!');
    const { provider, disposable } = CSVEditorProvider_1.CSVEditorProvider.register(context);
    context.subscriptions.push(disposable);
    context.subscriptions.push(vscode.commands.registerCommand('bigDataExplorer.openFolder', (uri) => {
        if (!uri) {
            vscode.window.showErrorMessage("No se seleccionó ninguna carpeta.");
            return;
        }
        provider.openFolder(uri);
    }));
    // Check for updates and show changelog
    const currentVersion = context.extension.packageJSON.version;
    const lastVersion = context.globalState.get('extensionVersion');
    if (currentVersion !== lastVersion) {
        vscode.window.showInformationMessage(`🚀 Forensic Nitro-Search actualizado a v${currentVersion}! ¿Quieres ver las novedades?`, 'Ver Novedades').then(selection => {
            if (selection === 'Ver Novedades') {
                const changelogPath = vscode.Uri.file(path.join(context.extensionPath, 'CHANGELOG.md'));
                vscode.commands.executeCommand('markdown.showPreview', changelogPath);
            }
        });
        context.globalState.update('extensionVersion', currentVersion);
    }
}
function deactivate() { }
//# sourceMappingURL=extension.js.map