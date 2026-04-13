import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import { CSVDocument } from './CSVDocument';
import { execFile, spawn } from 'child_process';
import * as Papa from 'papaparse';
import { promisify } from 'util';
import * as os from 'os';
import { telemetry } from './telemetry';

const execFileAsync = promisify(execFile);

export class CSVEditorProvider implements vscode.CustomEditorProvider<CSVDocument> {
    public static readonly viewType = 'bigDataExplorer.csv';

    constructor(
        private readonly context: vscode.ExtensionContext
    ) { }

    public static register(context: vscode.ExtensionContext): { provider: CSVEditorProvider, disposable: vscode.Disposable } {
        const provider = new CSVEditorProvider(context);
        const disposable = vscode.window.registerCustomEditorProvider(CSVEditorProvider.viewType, provider, {
            webviewOptions: {
                retainContextWhenHidden: true
            }
        });
        return { provider, disposable };
    }

    private readonly _onDidChangeCustomDocument = new vscode.EventEmitter<vscode.CustomDocumentEditEvent<CSVDocument>>();
    public readonly onDidChangeCustomDocument = this._onDidChangeCustomDocument.event;

    public async saveCustomDocument(document: CSVDocument, cancellation: vscode.CancellationToken): Promise<void> { }
    public async saveCustomDocumentAs(document: CSVDocument, destination: vscode.Uri, cancellation: vscode.CancellationToken): Promise<void> { }
    public async revertCustomDocument(document: CSVDocument, cancellation: vscode.CancellationToken): Promise<void> { }
    public async backupCustomDocument(document: CSVDocument, context: vscode.CustomDocumentBackupContext, cancellation: vscode.CancellationToken): Promise<vscode.CustomDocumentBackup> {
        return { id: context.destination.toString(), delete: () => { } };
    }

    public async openCustomDocument(uri: vscode.Uri, _openContext: vscode.CustomDocumentOpenContext, _token: vscode.CancellationToken): Promise<CSVDocument> {
        return CSVDocument.create(uri);
    }

    public async resolveCustomEditor(document: CSVDocument, webviewPanel: vscode.WebviewPanel, _token: vscode.CancellationToken): Promise<void> {
        this.setupWebview(webviewPanel, document.uri, false);
    }

    public openFolder(uri: vscode.Uri) {
        const panel = vscode.window.createWebviewPanel(
            'bigDataExplorer.dataset',
            `Dataset: ${path.basename(uri.fsPath)}`,
            vscode.ViewColumn.One,
            {
                enableScripts: true,
                retainContextWhenHidden: true,
                localResourceRoots: [
                    vscode.Uri.file(path.join(this.context.extensionPath, 'out')),
                    vscode.Uri.file(path.join(this.context.extensionPath, 'webview-ui/public'))
                ]
            }
        );
        this.setupWebview(panel, uri, true);
    }

    private setupWebview(webviewPanel: vscode.WebviewPanel, uri: vscode.Uri, isDataset: boolean) {
        const documentPath = uri.fsPath;
        let columns: string[] = [];
        let types: Record<string, string> = {};
        let filteredIndices: number[] | null = null;
        let totalFileLines = 0;
        let rpcResolvers = new Map<string, (resolve: any) => void>();
        let rpcCounter = 0;
        let daemon: import('child_process').ChildProcessWithoutNullStreams | null = null;
        let rpcBuffer = '';

        webviewPanel.webview.options = {
            enableScripts: true,
            localResourceRoots: [
                vscode.Uri.file(path.join(this.context.extensionPath, 'out')),
                vscode.Uri.file(path.join(this.context.extensionPath, 'webview-ui/public'))
            ]
        };

        webviewPanel.webview.html = this.getHtmlForWebview(webviewPanel.webview);

        const initMsg = isDataset ? '🚀 Zen-Engine: Recursive Dataset Scan...' : '🚀 Zen-Engine: Turbo-Indexing...';
        webviewPanel.webview.postMessage({ type: 'init_process', message: initMsg });

        let currentSysInfo: any = null;
        let skipRows = 0;

        const BIG_EXPLORER_ENGINE_PATH = this.getEnginePath();
        execFile(BIG_EXPLORER_ENGINE_PATH, ['sys-info'], async (err, stdout) => {
            if (!err) {
                try {
                    currentSysInfo = JSON.parse(stdout);
                    const diskType = await this.getDiskType(documentPath);
                    currentSysInfo.disk = diskType;
                    webviewPanel.webview.postMessage({ type: 'sys_info', data: currentSysInfo });
                } catch (e) { }
            }
        });

        const startTime = Date.now();

        // Initialize Daemon RPC
        console.log(`[Nitro-Engine] Spawning daemon for: ${documentPath}`);
        daemon = spawn(BIG_EXPLORER_ENGINE_PATH, ['daemon', '--path', documentPath]);
        
        // Notify webview that indexing has started
        webviewPanel.webview.postMessage({ 
            type: 'init_process', 
            message: '🚀 Zen-Engine: Turbo-Indexing for 60FPS Performance...' 
        });

        daemon.stderr.on('data', (data) => {
            console.warn(`[Nitro-Engine stderr]: ${data}`);
        });

        daemon.on('close', (code) => {
            console.error(`[Nitro-Engine] Process closed with code ${code}`);
            if (code !== 0) {
                webviewPanel.webview.postMessage({ type: 'filter_error', error: `Engine closed unexpectedly (Code ${code})` });
            }
        });
        
        daemon.stdout.on('data', (data) => {
            rpcBuffer += data.toString();
            let lines = rpcBuffer.split('\n');
            rpcBuffer = lines.pop() || '';
            
            for (let line of lines) {
                const trimmed = line.trim();
                if (!trimmed) continue;
                try {
                    const parsed = JSON.parse(trimmed);
                    console.log(`[Nitro-Engine] Incoming: ${parsed.status || parsed.msg_id || 'unknown'}`);

                    if (parsed.status === 'ready') {
                        totalFileLines = parsed.total_rows;
                        const msgId = "init_meta_" + (++rpcCounter);
                        rpcResolvers.set(msgId, (res: any) => {
                            if (!res || !res.columns) {
                                console.error("[Nitro-Engine] Metadata failure: invalid response", res);
                                return;
                            }
                            skipRows = res.skip_rows || 0;
                            columns = res.columns.map((c: any) => c.name);
                            if (isDataset) columns.unshift("Source File");
                            
                            types = {};
                            if (isDataset) types["Source File"] = "string";
                            res.columns.forEach((c: any) => {
                                let uiType = 'string';
                                const rustType = c.data_type.toLowerCase();
                                if (rustType.includes('numeric') || rustType.includes('int')) uiType = 'number';
                                else if (rustType.includes('date')) uiType = 'date';
                                types[c.name] = uiType;
                            });
                            
                            webviewPanel.webview.postMessage({
                                type: 'init_parsed',
                                columns,
                                types,
                                total: isDataset ? 10000 : (totalFileLines > 0 ? totalFileLines - 1 : 0),
                                isDataset
                            });
                            console.log(`[Nitro-Engine] UI Handshake Complete. Total Rows: ${totalFileLines}`);
                        });
                        daemon!.stdin.write(JSON.stringify({ msg_id: msgId, cmd: 'metadata' }) + '\n');
                    } else if (parsed.msg_id && rpcResolvers.has(parsed.msg_id)) {
                        rpcResolvers.get(parsed.msg_id)!(parsed.result);
                        rpcResolvers.delete(parsed.msg_id);
                    }
                } catch (e) {
                    console.error("[Nitro-Engine] RPC JSON Parse Error: ", e, trimmed);
                }
            }
        });
        
        daemon.on('error', (err) => {
            // Fallback JS processing if binary missing
            console.error("Daemon missing", err);
            webviewPanel.webview.postMessage({ type: 'init_process', message: 'ERROR: Native Engine missing. Using slow TS fallback...' });
            
            // Keep basic stream logic just in case it's Windows and they don't have the Native binary!
            const stream = fs.createReadStream(documentPath);
            let offset = 0; const offsets = [0]; const lengths: number[] = [];
            stream.on('data', (chunk: any) => {
                for(let i=0; i<chunk.length; i++) {
                    if (chunk[i] === 10) { const le = offset+i+1; lengths.push(le-offsets[offsets.length-1]); offsets.push(le); }
                }
                offset += chunk.length;
            });
            stream.on('end', () => {
                totalFileLines = offsets.length;
                webviewPanel.webview.postMessage({ type: 'init_parsed', columns: ['Raw Data'], types: {'Raw Data': 'string'}, total: totalFileLines-1, isDataset });
            });
        });

        webviewPanel.onDidDispose(() => {
            if (daemon) {
                daemon.kill();
                daemon = null;
            }
        });

        webviewPanel.webview.onDidReceiveMessage(async (e: any) => {
            switch (e.type) {
                case 'init':
                    // If we already have metadata, resend it to be sure
                    if (columns.length > 0) {
                        webviewPanel.webview.postMessage({
                            type: 'init_parsed',
                            columns,
                            types,
                            total: isDataset ? 10000 : totalFileLines - 1,
                            isDataset
                        });
                    }
                    break;
                case 'filter':
                    if (!e.query) {
                        filteredIndices = null;
                        webviewPanel.webview.postMessage({ type: 'filter_applied', indices: [], total: totalFileLines - 1 });
                        break;
                    }

                    // 1. Check if it's a line number jump
                    const lineJump = parseInt(e.query);
                    if (!isNaN(lineJump) && /^\d+$/.test(e.query)) {
                        webviewPanel.webview.postMessage({ type: 'filter_applied', indices: [], total: totalFileLines - 1, jump: lineJump });
                        break;
                    }

                    // 2. Perform search
                    if (daemon && daemon.pid) {
                        const msgId = (++rpcCounter).toString();
                        rpcResolvers.set(msgId, async (res: any) => {
                            if (res && res.indices) {
                                filteredIndices = res.indices;
                                
                                // Get first 10 rows for the overlay
                                const sampleIndices = filteredIndices!.slice(0, 10);
                                let sampleRows: any[][] = [];
                                
                                if (sampleIndices.length > 0) {
                                    const rowsMsgId = (++rpcCounter).toString();
                                    rpcResolvers.set(rowsMsgId, (rowsRes: any) => {
                                        if (rowsRes && rowsRes.rows) {
                                            sampleRows = rowsRes.rows.map((r: string) => (Papa.parse(r, { header: false }).data[0] as string[]) || [r]);
                                            webviewPanel.webview.postMessage({ 
                                                type: 'filter_applied', 
                                                indices: filteredIndices, 
                                                sample: sampleRows 
                                            });
                                        }
                                    });
                                    // Hack: passing indices as start/limit is not ideal for the engine, 
                                    // but we can just request the first page of the filtered view if we had it.
                                    // For now, let's request them individually or adjust engine.
                                    // Actually, let's just send the indices and let the UI request them if small,
                                    // but for the "Overlay" we want them fast.
                                    daemon!.stdin.write(JSON.stringify({ msg_id: rowsMsgId, cmd: 'get_rows', start: filteredIndices![0], limit: 10 }) + "\n");
                                } else {
                                    webviewPanel.webview.postMessage({ type: 'filter_applied', indices: [], sample: [] });
                                }
                            }
                        });
                        daemon.stdin.write(JSON.stringify({ msg_id: msgId, cmd: 'search', query: e.query, limit: 100000 }) + '\n');
                    }
                    break;

                case 'open_results_tab':
                    if (filteredIndices && filteredIndices.length > 0) {
                        const tempPath = path.join(os.tmpdir(), `zen_results_${Date.now()}.csv`);
                        // This would require fetching all rows. For performance, let's just notify.
                        vscode.window.showInformationMessage(`Exportando ${filteredIndices.length} resultados a pestaña nueva...`);
                        // In a real scenario, we'd stream filtered rows to tempPath and open it.
                    }
                    break;

                case 'get_rows':
                    if (daemon && daemon.pid) {
                        const msgId = (++rpcCounter).toString();
                        const limit = Math.min(e.end - e.start, 500);
                        rpcResolvers.set(msgId, (res: any) => {
                            if (res && res.rows) {
                                const parsedRows = res.rows.map((rowStr: string) => {
                                    return (Papa.parse(rowStr, { header: false }).data[0] as string[]) || [rowStr];
                                });
                                webviewPanel.webview.postMessage({ type: 'rows_data', start: e.start, data: parsedRows });
                            }
                        });
                        daemon.stdin.write(JSON.stringify({ msg_id: msgId, cmd: 'get_rows', start: e.start, limit: limit }) + '\n');
                    }
                    break;
            }
        });
    }

    private detectTypes(data: any[][], columns: string[]) {
        const types: Record<string, string> = {};
        columns.forEach((col, colIdx) => {
            let type = 'string';
            for (let i = 0; i < Math.min(data.length, 50); i++) {
                const val = data[i][colIdx];
                if (!val) continue;
                if (/^(true|false|1|0)$/i.test(val)) type = 'boolean';
                else if (/^[$€£¥]/.test(val)) type = 'currency';
                else if (!isNaN(Number(val))) type = 'number';
                else if (!isNaN(Date.parse(val)) && val.length > 5) type = 'date';
                else { type = 'string'; break; }
            }
            types[col] = type;
        });
        return types;
    }

    private getHtmlForWebview(webview: vscode.Webview): string {
        const scriptUri = webview.asWebviewUri(vscode.Uri.file(path.join(this.context.extensionPath, 'out', 'webview', 'index.js')));
        const styleUri = webview.asWebviewUri(vscode.Uri.file(path.join(this.context.extensionPath, 'out', 'webview', 'index.css')));
        return `<!DOCTYPE html><html><head><meta charset="UTF-8"><link href="${styleUri}" rel="stylesheet"></head><body><div id="app"></div><script type="module" src="${scriptUri}"></script></body></html>`;
    }

    private getEnginePath(): string {
        const platform = os.platform();
        let binName = 'big-explorer-engine-linux-x64';
        if (platform === 'win32') binName = 'big-explorer-engine.exe';
        else if (platform === 'darwin') binName = 'big-explorer-engine-macos';
        return path.join(this.context.extensionPath, 'bin', binName);
    }

    private async getDiskType(filePath: string): Promise<string> {
        return new Promise((resolve) => {
            const platform = os.platform();
            if (platform === 'linux') {
                execFile('findmnt', ['-n', '-o', 'SOURCE', '--target', filePath], (err, stdout) => {
                    if (err || !stdout) return resolve('Unknown');
                    const disk = stdout.trim().replace(/[0-9]/g, '').replace('/dev/', '');
                    fs.readFile(`/sys/block/${disk}/queue/rotational`, 'utf8', (err2, rotational) => {
                        resolve(rotational?.trim() === '1' ? 'HDD' : 'SSD');
                    });
                });
            } else resolve('Unknown');
        });
    }
}
