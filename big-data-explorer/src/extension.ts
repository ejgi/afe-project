import * as vscode from 'vscode';
import * as path from 'path';
import { CSVEditorProvider } from './CSVEditorProvider';

export function activate(context: vscode.ExtensionContext) {
  console.log('Big Data Explorer is now active!');
  
  const { provider, disposable } = CSVEditorProvider.register(context);
  context.subscriptions.push(disposable);

  context.subscriptions.push(
    vscode.commands.registerCommand('bigDataExplorer.openFolder', (uri: vscode.Uri) => {
        if (!uri) {
            vscode.window.showErrorMessage("No se seleccionó ninguna carpeta.");
            return;
        }
        provider.openFolder(uri);
    })
  );

  // Check for updates and show changelog
  const currentVersion = context.extension.packageJSON.version;
  const lastVersion = context.globalState.get<string>('extensionVersion');
  if (currentVersion !== lastVersion) {
    vscode.window.showInformationMessage(
      `🚀 Forensic Nitro-Search actualizado a v${currentVersion}! ¿Quieres ver las novedades?`,
      'Ver Novedades'
    ).then(selection => {
      if (selection === 'Ver Novedades') {
        const changelogPath = vscode.Uri.file(path.join(context.extensionPath, 'CHANGELOG.md'));
        vscode.commands.executeCommand('markdown.showPreview', changelogPath);
      }
    });
    context.globalState.update('extensionVersion', currentVersion);
  }
}

export function deactivate() {}
