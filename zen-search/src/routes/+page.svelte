<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import logo from "../assets/zen_ioc_logo.png";

  interface FileHit {
    path: string;
    count: number;
  }

  interface IpFrequency {
    ip: string;
    count: number;
    country_code?: string;
    country_name?: string;
    is_noise: boolean;
    top_files: FileHit[];
  }

  interface IpScanResult {
    results: IpFrequency[];
    total_unique: number;
    total_hits: number;
    truncated: boolean;
  }

  let folders = $state<string[]>([]);
  let hardwareMode = $state<"ssd" | "hdd">("ssd");
  let results = $state<IpFrequency[]>([]);
  let maxCount = $state(0);
  let isScanning = $state(false);
  let scanTimeMs = $state(0);
  let totalIps = $state(0);
  let uniqueIps = $state(0);
  let isTruncated = $state(false);
  let expandedIps = $state<Record<number, boolean>>({});
  let hidePrivate = $state(false);
  let resultsFilter = $state("");
  let sortOrder = $state<"hits" | "ip">("hits");
  let radarMode = $state<"v4" | "v6" | "both">("both");
  let showNoise = $state(false);

  // Virtual Scrolling State
  let scrollTop = $state(0);
  let viewportHeight = $state(600);
  const rowHeight = 46; // Base height of a row
  const buffer = 5;

  const virtualItems = $derived.by(() => {
     const startIdx = Math.max(0, Math.floor(scrollTop / rowHeight) - buffer);
     const endIdx = Math.min(filteredResults.length, Math.ceil((scrollTop + viewportHeight) / rowHeight) + buffer);
     
     return filteredResults.slice(startIdx, endIdx).map((item, i) => ({
        ...item,
        originalIdx: startIdx + i,
        top: (startIdx + i) * rowHeight
     }));
  });

  function handleScroll(e: UIEvent) {
    scrollTop = (e.target as HTMLElement).scrollTop;
  }

  function getFlagEmoji(countryCode: string | undefined): string {
    if (!countryCode) return '🏳️';
    const codePoints = countryCode
      .toUpperCase()
      .split('')
      .map(char => 127397 + char.charCodeAt(0));
    return String.fromCodePoint(...codePoints);
  }

  function isPrivate(ip: string): boolean {
    const parts = ip.split('.').map(Number);
    if (parts.length !== 4) return false;
    // 127.0.0.1 (Loopback)
    if (parts[0] === 127) return true;
    // 10.0.0.0 – 10.255.255.255
    if (parts[0] === 10) return true;
    // 172.16.0.0 – 172.31.255.255
    if (parts[0] === 172 && parts[1] >= 16 && parts[1] <= 31) return true;
    // 192.168.0.0 – 192.168.255.255
    if (parts[0] === 192 && parts[1] === 168) return true;
    // 169.254.x.x (APIPA)
    if (parts[0] === 169 && parts[1] === 254) return true;
    return false;
  }

  const filteredResults = $derived(
    results
      .filter(r => !hidePrivate || !isPrivate(r.ip))
      .filter(r => showNoise || !r.is_noise)
      .filter(r => r.ip.includes(resultsFilter))
      .sort((a, b) => {
        if (sortOrder === "hits") return b.count - a.count;
        // Generic IP sort (v4/v6)
        if (a.ip.includes(':') || b.ip.includes(':')) {
           return a.ip.localeCompare(b.ip); // Simple lex sort for v6
        }
        const ipA = a.ip.split('.').map(n => n.padStart(3, '0')).join('.');
        const ipB = b.ip.split('.').map(n => n.padStart(3, '0')).join('.');
        return ipA.localeCompare(ipB);
      })
  );

  async function loadSettings() {
    try {
      const s: any = await invoke("get_settings");
      folders = s.folders ?? [];
      hardwareMode = s.hardware_mode === "hdd" ? "hdd" : "ssd";
    } catch (e) {
      console.error(e);
    }
  }

  async function saveSettingsLocally() {
    await invoke("save_settings", { settings: { folders, hardware_mode: hardwareMode, max_results: 200 } });
  }

  async function removeFolder(i: number) {
    folders = folders.filter((_, idx) => idx !== i);
    await saveSettingsLocally();
  }

  async function browseFolder() {
    const selectedPath = await open({
      directory: true,
      multiple: false,
      title: "Select Data Source"
    });
    
    if (selectedPath && typeof selectedPath === 'string') {
      if (!folders.includes(selectedPath)) {
        folders = [...folders, selectedPath];
        await saveSettingsLocally();
      }
    }
  }

  async function browseFile() {
    const selectedPath = await open({
      directory: false,
      multiple: false,
      title: "Select Data File"
    });
    
    if (selectedPath && typeof selectedPath === 'string') {
      if (!folders.includes(selectedPath)) {
        folders = [...folders, selectedPath];
        await saveSettingsLocally();
      }
    }
  }

  async function startScan() {
    if (folders.length === 0) return;
    isScanning = true;
    results = [];
    expandedIps = {};
    maxCount = 0;
    totalIps = 0;
    uniqueIps = 0;
    isTruncated = false;
    scanTimeMs = 0;
    const start = performance.now();

    try {
      const data = await invoke<IpScanResult>("extract_ips_cmd", { mode: radarMode });
      scanTimeMs = performance.now() - start;
      results = data.results;
      uniqueIps = data.total_unique;
      totalIps = data.total_hits;
      isTruncated = data.truncated;
      
      if (results.length > 0) {
        maxCount = results[0].count; // Already sorted DESC
      }
    } catch (e) {
      console.error("Scan error:", e);
      alert(String(e));
    } finally {
      isScanning = false;
    }
  }

  async function cancelScan() {
    try {
      await invoke("cancel_scan");
    } catch (e) {
      console.error("Cancel error:", e);
    }
  }

  async function exportCsv() {
    try {
      const filePath = await save({
        filters: [{
          name: 'CSV Report',
          extensions: ['csv']
        }],
        title: "Export IOC Report"
      });

      if (!filePath) return;

      let csvContent = "IP Address,Total Hits,Origins\n";
      for (const ip of results) {
        const origins = ip.top_files.map(f => `${f.path} (${f.count})`).join(" | ");
        csvContent += `"${ip.ip}",${ip.count},"${origins}"\n`;
      }

      await invoke("save_report_cmd", { path: filePath, content: csvContent });
      alert("Report saved successfully!");
    } catch (e) {
      alert("Could not export report: " + e);
    }
  }

  function toggleExpand(idx: number, e: MouseEvent | KeyboardEvent) {
    expandedIps[idx] = !expandedIps[idx];
  }

  function copyText(txt: string, e: MouseEvent | KeyboardEvent) {
    if (e instanceof MouseEvent) e.stopPropagation();
    navigator.clipboard.writeText(txt);
  }

  function lookupVT(ip: string, e: MouseEvent) {
    e.stopPropagation();
    const url = `https://www.virustotal.com/gui/ip-address/${ip}`;
    invoke("open_file", { path: url }); // Note: open_file works for URLs too via xdg-open/open/start
  }

  function lookupAbuse(ip: string, e: MouseEvent) {
    e.stopPropagation();
    const url = `https://www.abuseipdb.com/check/${ip}`;
    invoke("open_file", { path: url });
  }

  onMount(async () => {
    await loadSettings();
  });
</script>

<div class="app-container">
  <!-- HEADER -->
  <header class="header" data-tauri-drag-region>
    <div class="logo-section">
      <img src={logo} alt="ZEN-IOC Logo" class="logo-img" />
      <div class="logo-text">
        <h1 class="logo-main">Z-IOC</h1>
        <span class="logo-sub">Massive IP Extractor</span>
        <span class="logo-slogan">See what others miss. Find threats at machine speed.</span>
      </div>
    </div>
  </header>

  <!-- MAIN GRID -->
  <div class="main-grid">
    
    <!-- LEFT CONTROLS -->
    <aside class="controls-panel">
      <!-- Action Button -->
      <button 
        class="extract-btn {isScanning ? 'scanning' : ''}" 
        disabled={isScanning || folders.length === 0}
        onclick={startScan}>
        {isScanning ? 'SCANNING DATA SET...' : 'EXTRACT NETWORK IOCs'}
      </button>

      {#if isScanning}
        <button 
          class="cancel-btn"
          onclick={cancelScan}>
          ⏹ STOP SCANNING
        </button>
      {/if}

      <!-- Folders Box -->
      <div class="glass-box">
        <h3 class="box-title">Target Sources</h3>
        <div class="folder-list">
          {#each folders as f, i}
            <div class="folder-item">
              <span class="folder-path" title={f}>{f}</span>
              <button class="remove-btn" onclick={() => removeFolder(i)}>✕</button>
            </div>
          {/each}
        </div>
        <div style="display: flex; gap: 8px;">
          <button class="add-folder-btn" style="flex:1;" onclick={browseFolder}>
            + Add Directory
          </button>
          <button class="add-folder-btn" style="flex:1;" onclick={browseFile}>
            + Add File
          </button>
        </div>
        {#if folders.length > 0}
          <button class="remove-btn" style="font-size: 10px; align-self: flex-end;" onclick={() => { folders = []; saveSettingsLocally(); }}>
            Clear All Sources
          </button>
        {/if}
      </div>

      <!-- Settings Box -->
      <div class="glass-box">
        <h3 class="box-title">Filters & Triage</h3>
        
        <div style="margin-bottom: 12px;">
          <input 
            type="text" 
            placeholder="Search IPs (e.g. 192.168...)" 
            bind:value={resultsFilter}
            style="width: 100%; background: rgba(0,0,0,0.3); border: 1px solid rgba(255,255,255,0.1); border-radius: 8px; padding: 10px; color: white; font-size: 11px; font-family: 'JetBrains Mono', monospace;"
          />
        </div>

        <div style="display: flex; flex-direction: column; gap: 8px;">
          <label style="display: flex; align-items: center; gap: 10px; font-size: 13px; color: #cbd5e1; cursor: pointer;">
            <input type="checkbox" bind:checked={hidePrivate} style="accent-color: #a855f7;">
            Hide Private/Local IPs 
          </label>
          <label style="display: flex; align-items: center; gap: 10px; font-size: 13px; color: #a855f7; cursor: pointer;">
            <input type="checkbox" bind:checked={showNoise} style="accent-color: #a855f7;">
            Show Potential Noise
          </label>
        </div>

        <div style="margin-top: 12px; display: flex; flex-direction: column; gap: 8px;">
          <span style="font-size: 11px; color: #a855f7; font-family: 'JetBrains Mono', monospace; text-transform: uppercase;">Sort Order</span>
          <div class="hw-mode-selector">
            <button class="hw-btn {sortOrder === 'hits' ? 'active' : ''}" onclick={() => sortOrder = 'hits'}>Top Hits</button>
            <button class="hw-btn {sortOrder === 'ip' ? 'active' : ''}" onclick={() => sortOrder = 'ip'}>IP Address</button>
          </div>
        </div>

        <div style="margin-top: 12px; display: flex; flex-direction: column; gap: 8px;">
          <span style="font-size: 11px; color: #a855f7; font-family: 'JetBrains Mono', monospace; text-transform: uppercase;">Radar Mode</span>
          <div class="hw-mode-selector">
            <button class="hw-btn {radarMode === 'v4' ? 'active' : ''}" onclick={() => radarMode = 'v4'}>IPv4</button>
            <button class="hw-btn {radarMode === 'v6' ? 'active' : ''}" onclick={() => radarMode = 'v6'}>IPv6</button>
            <button class="hw-btn {radarMode === 'both' ? 'active' : ''}" onclick={() => radarMode = 'both'}>Dual</button>
          </div>
        </div>
        
        <h3 class="box-title" style="margin-top: 8px;">Hardware Mode</h3>
        <div class="hw-mode-selector">
          <button 
            class="hw-btn {hardwareMode === 'ssd' ? 'active' : ''}"
            onclick={() => { hardwareMode = 'ssd'; saveSettingsLocally(); }}>
            M.2 / SSD
          </button>
          <button 
            class="hw-btn {hardwareMode === 'hdd' ? 'active' : ''}"
            onclick={() => { hardwareMode = 'hdd'; saveSettingsLocally(); }}>
            HDD (Stream)
          </button>
        </div>
      </div>
    </aside>

    <!-- RIGHT RESULTS -->
    <main class="results-panel">
      
      {#if scanTimeMs > 0 && !isScanning}
        <div class="status-bar">
          <div class="stats">
            <span>⏱ {(scanTimeMs/1000).toFixed(2)}s</span>
            <span>🌐 {uniqueIps.toLocaleString()} Unique IPs</span>
            <span>📊 {totalIps.toLocaleString()} Total Hits</span>
            {#if results.length > 0}
               <button class="export-btn" onclick={exportCsv}>💾 Export CSV</button>
            {/if}
          </div>
          {#if isTruncated}
            <div class="truncation-notice">
              <span>⚠️</span>
              <span>Showing top 1,000 of {uniqueIps.toLocaleString()} unique IPs — Export CSV for full dataset.</span>
            </div>
          {/if}
          
          {#if !showNoise && results.length > filteredResults.length}
            <div class="noise-notification">
              <span>🛡️ Smart Filter:</span>
              <span>{results.length - filteredResults.length} potential noise hits hidden.</span>
              <button class="text-btn" onclick={() => showNoise = true}>Reveal Hits</button>
            </div>
          {/if}
        </div>
      {/if}

      <div class="table-header">
        <div class="col-rank">Rank</div>
        <div class="col-ip">IP Address ({radarMode.toUpperCase()})</div>
        <div class="col-count">Hits</div>
        <div class="col-freq">Frequency Analytics</div>
        <div class="col-actions"></div>
      </div>

      <div class="results-list" onscroll={handleScroll}>
        {#if isScanning}
          <div class="empty-state">
            <div class="radar"></div>
            <p>Carving raw data blocks for topological anomalies...</p>
            <button class="cancel-btn-ghost" onclick={cancelScan}>Abort Process</button>
          </div>
        {:else if results.length === 0 && scanTimeMs === 0}
          <div class="empty-state">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/>
            </svg>
            <p>Select target directories/files and initiate extraction.</p>
          </div>
        {:else if results.length === 0 && scanTimeMs > 0}
          <div class="empty-state">
            <p>Scan complete. No IPs found in the dataset.</p>
          </div>
        {:else}
          <div class="virtual-stretcher" style="height: {filteredResults.length * rowHeight}px; position: relative;">
            {#each virtualItems as r}
               <div class="result-row" class:expanded={expandedIps[r.originalIdx]} 
                    class:noise-row={r.is_noise}
                    style="position: absolute; top: {r.top}px; width: 100%; height: {expandedIps[r.originalIdx] ? 'auto' : rowHeight + 'px'}; z-index: {expandedIps[r.originalIdx] ? 10 : 1}; background: {expandedIps[r.originalIdx] ? 'rgba(30, 41, 59, 0.95)' : 'transparent'}">
                 <div class="result-main" role="button" tabindex="0" onclick={(e) => toggleExpand(r.originalIdx, e)} onkeydown={(e) => e.key === 'Enter' && toggleExpand(r.originalIdx, e)}>
                   <div class="col-rank">#{r.originalIdx + 1}</div>
                   <div class="col-ip">
                     <span class="flag" title={r.country_name || 'Unknown'} style="margin-right: 6px; font-size: 14px;">
                        {getFlagEmoji(r.country_code)}
                     </span>
                     {r.ip}
                     {#if r.is_noise}
                       <span class="noise-badge" title="Detected in binary noise context">NOISE</span>
                     {/if}
                     <!-- svelte-ignore a11y_click_events_have_key_events -->
                     <!-- svelte-ignore a11y_no_static_element_interactions -->
                     <div class="ip-actions" onclick={(e) => e.stopPropagation()}>
                       <button type="button" class="mini-btn copy" title="Copy IP" onclick={(e) => copyText(r.ip, e)}>📋</button>
                       <button type="button" class="mini-btn vt" title="VirusTotal Lookup" onclick={(e) => lookupVT(r.ip, e)}>🛡️</button>
                       <button type="button" class="mini-btn abuse" title="AbuseIPDB Check" onclick={(e) => lookupAbuse(r.ip, e)}>🚫</button>
                     </div>
                   </div>
                   <div class="col-count">{r.count.toLocaleString()}</div>
                   <div class="col-freq">
                     <div class="freq-bar-container">
                       <div class="freq-bar" style="width: {(r.count / maxCount) * 100}%"></div>
                     </div>
                   </div>
                   <div class="col-actions">
                      <span class="chevron">{expandedIps[r.originalIdx] ? '▲' : '▼'}</span>
                   </div>
                 </div>
                 
                 <!-- Trazabilidad: Origin files -->
                 {#if expandedIps[r.originalIdx]}
                   <div class="result-traces">
                     <div class="traces-header">Tracked File Origins (Top {r.top_files.length})</div>
                     {#each r.top_files as f}
                        <div class="trace-item">
                           <span class="trace-icon">📄</span>
                           <span class="trace-path" title={f.path}>{f.path}</span>
                           <span class="trace-badge">{f.count} hits</span>
                        </div>
                     {/each}
                   </div>
                 {/if}
               </div>
            {/each}
          </div>
        {/if}
      </div>

    </main>
  </div>
</div>
