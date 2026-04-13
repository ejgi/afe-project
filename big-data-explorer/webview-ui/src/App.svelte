<script lang="ts">
  import { onMount, tick } from 'svelte';
  import VirtualTable from './components/VirtualTable.svelte';

  let columns: string[] = [];
  let types: Record<string, string> = {};
  let totalRows = 0;
  let loading = true;
  let status = "Waiting for file...";
  let query = "";
  let sysInfo: any = null;
  let showPrivacy = false;
  let vscodeApi: any;
  
  let resultsData: string[][] = [];
  let showResultsOverlay = false;
  let originalTotal = 0;
  let rowCache = new Map<number, string[]>();
  
  // For throttling search
  let searchTimeout: any;

  let ignoreCase = true; // Default to case-insensitive
  let loadingMessage = "Nitro-Engine Indexing...";
  let pendingRequests = new Set<string>();
  let matchIndices: number[] = [];
  let currentMatchIdx = -1;
  let targetIndex: number | undefined;
  let lastQuery = "";

  function onKeyDown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
        executeSearch();
    }
  }

  function executeSearch() {
    lastQuery = query;
    if (vscodeApi) {
        vscodeApi.postMessage({ type: 'filter', query, ignoreCase });
        status = "Analizando...";
        loadingMessage = "Nitro-Engine Analizando...";
        loading = true;
    }
  }

  function clearSearch() {
    query = "";
    executeSearch();
  }

  onMount(() => {
    try {
      // @ts-ignore
      vscodeApi = acquireVsCodeApi();
    } catch (e) {
      console.warn("VS Code API not available");
    }

    window.addEventListener('message', event => {
      const message = event.data;
      if (message.type === 'init_process') {
        loading = true;
        status = message.message;
      } else if (message.type === 'init_parsed') {
        columns = message.columns;
        types = message.types;
        totalRows = message.total;
        originalTotal = totalRows; // Keep original count
        loading = false;
        status = `Loaded ${totalRows} rows`;
      } else if (message.type === 'rows_data') {
        const { start, data } = message;
        pendingRequests.delete(`${start}-${start + data.length}`);
        for (let i = 0; i < data.length; i++) {
           rowCache.set(start + i, data[i]);
        }
        rowCache = rowCache; // trigger Svelte reactivity
      } else if (message.type === 'filter_start') {
        status = "Filtering... (this may take a while)";
      } else if (message.type === 'filter_applied') {
        const indices = message.indices || [];
        matchIndices = indices;
        
        // 1. Manejar Salto a Línea (Jump)
        if (message.jump !== undefined) {
             targetIndex = message.jump;
             status = `🚀 Saltando a línea ${message.jump}`;
             showResultsOverlay = false;
             loading = false;
             // Unset targetIndex after jump to allow re-triggering if same number
             setTimeout(() => { targetIndex = undefined; }, 100);
             return;
        }

        // 2. Manejar Resultados de Búsqueda
        if (matchIndices.length === 0 && query) {
            status = "❌ No se encontraron coincidencias";
            showResultsOverlay = false;
        } else if (matchIndices.length > 500) {
            status = `🚀 Coincidencias masivas (${matchIndices.length}). Mostrando pre-vista...`;
            resultsData = message.sample || [];
            showResultsOverlay = true;
            // Opcional: vscodeApi.postMessage({ type: 'open_results_tab' });
        } else if (matchIndices.length > 0) {
            status = `✅ Encontradas ${matchIndices.length} filas`;
            resultsData = message.sample || [];
            showResultsOverlay = true;
        } else {
            status = `Archivo listo (${originalTotal} filas)`;
            showResultsOverlay = false;
        }
        
        loading = false;
      }
 else if (message.type === 'filter_error') {
        status = `Error: ${message.error}`;
        loading = false;
      } else if (message.type === 'sys_info') {
        sysInfo = message.data;
      }
    });

    if (vscodeApi) {
      vscodeApi.postMessage({ type: 'init' });
    }
  });

  function requestRows(start: number, end: number) {
      const key = `${start}-${end}`;
      if (pendingRequests.has(key)) return;
      
      if (vscodeApi) {
          pendingRequests.add(key);
          vscodeApi.postMessage({ type: 'get_rows', start, end });
      }
  }
</script>

<main>
  <div class="header">
    <div class="title-group">
      <div class="title-row">
          <h2>Big Data Explorer</h2>
          <div class="zen-dash">
              <button class="zen-badge" on:click={() => showPrivacy = !showPrivacy}>
              ZEN-ENGINE ACTIVE ⓘ
          </button>
              {#if sysInfo}
                  <div class="sys-metrics">
                      <span class="metric"><b>CPU:</b> {sysInfo.cpu.split('@')[0]} ({sysInfo.cores} cores)</span>
                      <span class="metric"><b>RAM:</b> {(sysInfo.ram_mb / 1024).toFixed(1)}GB</span>
                      <span class="metric-nitro">NITRO: ON</span>
                  </div>
              {/if}
          </div>
      </div>
      <span class="status">{status}</span>
    </div>
    <div class="actions">
      <div class="search-container">
        <input 
          type="text" 
          placeholder="Search for text... (Enter to start)" 
          bind:value={query}
          on:keydown={onKeyDown}
          disabled={loading}
          class="search-input"
        />
      </div>
      <div class="search-options">
        <label>
          <input type="checkbox" bind:checked={ignoreCase} disabled={loading} />
          Abc (Ignorar Mayúsculas)
        </label>
      </div>
      <button class="search-btn" on:click={executeSearch} disabled={loading}>
        {loading ? 'Buscando...' : '🔍 Buscar'}
      </button>
    </div>
  </div>
  
  <VirtualTable 
    {totalRows} 
    {columns} 
    {types} 
    {rowCache} 
    {requestRows} 
    {loading}
    {loadingMessage}
    {targetIndex}
    {matchIndices}
  />
  
  {#if showResultsOverlay}
    <div class="results-viewfinder" role="region" aria-label="Resultados de búsqueda">
        <div class="viewfinder-header">
            <span>🔍 Coincidencias en Pantalla ({matchIndices.length})</span>
            <button class="close-overlay" on:click={() => showResultsOverlay = false} aria-label="Cerrar resultados">×</button>
        </div>
        <div class="viewfinder-body">
            <table>
                <thead>
                    <tr>{#each columns as col}<th>{col}</th>{/each}</tr>
                </thead>
                <tbody>
                    {#each resultsData as row}
                        <tr>{#each row as cell}<td>{cell}</td>{/each}</tr>
                    {/each}
                </tbody>
            </table>
        </div>
    </div>
  {/if}
  {#if showPrivacy}
    <div 
        class="privacy-modal" 
        on:click={(e) => e.target === e.currentTarget && (showPrivacy = false)} 
        on:keydown={(e) => e.key === 'Escape' && (showPrivacy = false)}
        role="button" 
        tabindex="0"
        aria-label="Cerrar Privacidad"
    >
        <div class="privacy-content">
            <h3>🛡️ Zen-Transparency</h3>
            <p>Para mejorar tu experiencia, recolectamos métricas de rendimiento anónimas:</p>
            <ul>
                <li><b>Hardware:</b> CPU, RAM y Sistema Operativo.</li>
                <li><b>Rendimiento:</b> Velocidad de búsqueda/indexado y tamaño de archivos.</li>
            </ul>
            <p class="privacy-note">⚠️ No recolectamos contenidos, nombres de archivos privados ni datos personales.</p>
            <button class="close-btn" on:click={() => showPrivacy = false}>Cerrar</button>
        </div>
    </div>
  {/if}
</main>

<style>
  :global(body) {
    padding: 0;
    margin: 0;
    overflow: hidden;
  }
  main {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
    background-color: var(--vscode-editor-background);
    color: var(--vscode-editor-foreground);
  }
  .header {
    padding: 12px 24px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    background: rgba(30, 30, 30, 0.7);
    backdrop-filter: blur(8px);
    border-bottom: 2px solid var(--zen-green-dim);
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.3);
  }
  .title-group {
    display: flex;
    flex-direction: column;
  }
  .title-row {
      display: flex;
      align-items: center;
      gap: 12px;
  }
  .zen-dash {
      display: flex;
      align-items: center;
      gap: 16px;
  }
  .sys-metrics {
      display: flex;
      gap: 12px;
      font-size: 0.75rem;
      background: rgba(0, 0, 0, 0.4);
      padding: 4px 12px;
      border-radius: 6px;
      border: 1px solid rgba(0, 255, 136, 0.2);
  }
  .metric {
      color: rgba(255, 255, 255, 0.7);
  }
  .metric b {
      color: #00ccff;
  }
  .metric-nitro {
      color: #00ff88;
      font-weight: 800;
      text-shadow: 0 0 8px rgba(0, 255, 136, 0.5);
  }
  .zen-badge {
      background: linear-gradient(135deg, #00ff88, #00ccff);
      color: #000;
      font-size: 0.7rem;
      font-weight: 800;
      padding: 2px 8px;
      border-radius: 12px;
      text-transform: uppercase;
      letter-spacing: 0.5px;
      box-shadow: 0 0 10px rgba(0, 255, 136, 0.4);
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .search-container {
    position: relative;
    display: flex;
    align-items: center;
  }
  .search-input {
    background-color: rgba(255, 255, 255, 0.05);
    color: #fff;
    border: 1px solid rgba(0, 255, 136, 0.3);
    padding: 8px 30px 8px 14px;
    border-radius: 4px;
    width: 300px;
    font-family: var(--vscode-font-family);
    transition: all 0.2s ease;
  }
  .search-input:focus {
    outline: none;
    border-color: #00ff88;
    box-shadow: 0 0 15px rgba(0, 255, 136, 0.2);
    width: 350px;
  }
  .search-btn {
    background: linear-gradient(135deg, rgba(0, 255, 136, 0.2), rgba(0, 204, 255, 0.2));
    color: #fff;
    border: 1px solid var(--zen-green-dim);
    padding: 8px 16px;
    border-radius: 4px;
    cursor: pointer;
    font-weight: 600;
    transition: all 0.2s ease;
  }
  .search-btn:hover:not(:disabled) {
    background: linear-gradient(135deg, rgba(0, 255, 136, 0.4), rgba(0, 204, 255, 0.4));
    border-color: #00ff88;
    box-shadow: 0 0 10px rgba(0, 255, 136, 0.3);
  }
  .search-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  h2 {
    margin: 0;
    font-size: 1.3rem;
    font-weight: 700;
    background: linear-gradient(135deg, #00ff88, #00ccff);
    -webkit-background-clip: text;
    background-clip: text;
    -webkit-text-fill-color: transparent;
  }
  
  /* Results Viewfinder Overlay */
  .results-viewfinder {
      position: absolute;
      bottom: 40px;
      right: 40px;
      width: 80%;
      height: 300px;
      background: rgba(15, 15, 20, 0.95);
      border: 1px solid var(--zen-green);
      border-radius: 8px;
      box-shadow: 0 10px 40px rgba(0,0,0,0.5);
      display: flex;
      flex-direction: column;
      z-index: 1000;
      backdrop-filter: blur(10px);
      animation: slideIn 0.3s ease-out;
  }
  @keyframes slideIn {
      from { transform: translateY(20px); opacity: 0; }
      to { transform: translateY(0); opacity: 1; }
  }
  .viewfinder-header {
      padding: 10px 20px;
      background: rgba(0, 255, 136, 0.1);
      border-bottom: 1px solid rgba(0, 255, 136, 0.2);
      display: flex;
      justify-content: space-between;
      align-items: center;
      color: var(--zen-green);
      font-weight: bold;
  }
  .close-overlay {
      background: none;
      border: none;
      color: #fff;
      font-size: 1.5rem;
      cursor: pointer;
  }
  .viewfinder-body {
      flex: 1;
      overflow: auto;
      padding: 10px;
  }
  .viewfinder-body table {
      width: 100%;
      border-collapse: collapse;
      font-size: 0.85rem;
  }
  .viewfinder-body th {
      text-align: left;
      padding: 8px;
      background: rgba(255,255,255,0.05);
      position: sticky;
      top: 0;
  }
  .viewfinder-body td {
      padding: 8px;
      border-bottom: 1px solid rgba(255,255,255,0.05);
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
      max-width: 200px;
  }
  .viewfinder-body tr:hover {
      background: rgba(0, 255, 136, 0.05);
  }
  .status {
    color: var(--vscode-descriptionForeground);
    font-size: 0.9rem;
  }

  /* Privacy Modal */
  .privacy-modal {
      position: fixed;
      top: 0;
      left: 0;
      width: 100vw;
      height: 100vh;
      background: rgba(0, 0, 0, 0.6);
      backdrop-filter: blur(4px);
      display: flex;
      align-items: center;
      justify-content: center;
      z-index: 1000;
  }
  .privacy-content {
      background: rgba(30, 30, 30, 0.95);
      border: 1px solid var(--zen-green-dim);
      padding: 24px;
      border-radius: 12px;
      max-width: 400px;
      box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5);
      border: 1px solid rgba(0, 255, 136, 0.3);
  }
  .privacy-content h3 {
      margin-top: 0;
      color: #00ff88;
  }
  .privacy-content p {
      font-size: 0.9rem;
      line-height: 1.5;
  }
  .privacy-content ul {
      font-size: 0.85rem;
      padding-left: 20px;
  }
  .privacy-note {
      font-size: 0.8rem;
      color: var(--vscode-descriptionForeground);
      font-style: italic;
      margin-top: 16px;
  }
  .close-btn {
      background: #333;
      color: #fff;
      border: none;
      padding: 8px 16px;
      border-radius: 4px;
      cursor: pointer;
      margin-top: 16px;
      width: 100%;
  }
  .close-btn:hover {
      background: #444;
  }
  .zen-badge {
      cursor: pointer;
      transition: transform 0.2s ease;
      border: none;
      font-family: inherit;
  }
  .zen-badge:hover {
      transform: scale(1.05);
  }
</style>
