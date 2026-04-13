import * as vscode from 'vscode';

export class CSVDocument implements vscode.CustomDocument {
    static async create(
        uri: vscode.Uri
    ): Promise<CSVDocument> {
        return new CSVDocument(uri);
    }

    private readonly _uri: vscode.Uri;

    private constructor(
        uri: vscode.Uri
    ) {
        this._uri = uri;
    }

    public get uri() { return this._uri; }

    public dispose(): void {
        // No-op for now. 
    }
}
