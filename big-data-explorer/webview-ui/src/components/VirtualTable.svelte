<script lang="ts">
  import VirtualList from 'svelte-tiny-virtual-list';

  export let totalRows = 0;
  export let columns: string[] = [];
  export let types: Record<string, string> = {};
  export let rowCache: Map<number, string[]>;
  export let requestRows: (start: number, end: number) => void;
  export let loading: boolean = false;
  export let loadingMessage: string = "Nitro-Engine Filtering...";
  export let targetIndex: number | undefined = undefined;
  export let matchIndices: number[] = [];
  export let itemHeight = 35;

  $: height = window.innerHeight - 100; // Account for header space

  let fetchQueue = new Set<number>();
  let fetchTimeout: any;

  function queueFetch(index: number) {
     fetchQueue.add(index);
     clearTimeout(fetchTimeout);
     fetchTimeout = setTimeout(() => {
        if (fetchQueue.size > 0) {
           const arr = Array.from(fetchQueue);
           const min = Math.min(...arr);
           const max = Math.max(...arr) + 15; // Prefetch a bit ahead
           requestRows(min, max);
           fetchQueue.clear();
        }
      }, 100); // 100ms debounce for 1GB+ stability
     return '';
  }
</script>

<div class="table-container" style="height: {height}px;">
  {#if totalRows === 0 && !loading}
    <div class="empty-state">No data loaded or empty dataset.</div>
  {:else}
    <!-- Header Row -->
    <div class="header-row">
      {#each columns as col}
        <div class="cell header-cell">{col}</div>
      {/each}
    </div>
    
    <!-- Virtual Body -->
    <div class="table-body-wrapper {loading && totalRows === 0 ? 'full-loading' : loading ? 'dimmed' : ''}">
      <VirtualList 
        width="100%" 
        height={height - itemHeight} 
        itemCount={totalRows} 
        itemSize={itemHeight}
        scrollToIndex={targetIndex}
        scrollToAlignment="center"
      >
        <div slot="item" let:index let:style {style} class="row {matchIndices.includes(index) ? 'highlight' : ''}">
          {#if rowCache.has(index)}
              {#each columns as col, colIdx}
                <div class="cell type-{types[col] || 'string'}" title={rowCache.get(index)?.[colIdx] ?? ''}>
                  {rowCache.get(index)?.[colIdx] ?? ''}
                </div>
              {/each}
          {:else}
              {queueFetch(index)}
              <div class="cell placeholder">...</div>
          {/if}
        </div>
      </VirtualList>
      
      <!-- The loading overlay is now always present but its visibility is controlled by the 'dimmed' class -->
      {#if loading}
        <div class="inner-loader">
            <div class="spinner"></div>
            <span>{loadingMessage}</span>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .table-container {
    width: 100%;
    overflow-x: auto; /* for many columns */
    display: flex;
    flex-direction: column;
    border: 1px solid var(--vscode-editorGroup-border);
  }

  .header-row {
    display: flex;
    font-weight: bold;
    background: var(--vscode-editorGroupHeader-tabsBackground);
    border-bottom: 2px solid var(--vscode-editorGroup-border);
    position: sticky;
    top: 0;
    z-index: 10;
  }

  .row {
    display: flex;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    transition: background 0.1s ease;
  }
  
  .row:hover {
    background: var(--zen-green-dim);
  }

  .row.highlight {
      background: rgba(255, 255, 0, 0.15) !important;
      border-left: 4px solid #ffcc00;
  }

  .cell {
    flex: 1;
    min-width: 120px;
    padding: 8px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  
  .header-cell {
    color: var(--vscode-foreground);
  }

  .empty-state {
    padding: 2rem;
    text-align: center;
    color: var(--vscode-descriptionForeground);
  }
  
  .placeholder {
    color: var(--vscode-descriptionForeground);
    font-style: italic;
    opacity: 0.5;
  }
  
  .table-body-wrapper {
      position: relative;
      flex: 1;
      height: 100%;
  }

  .table-body-wrapper.dimmed {
      opacity: 0.5;
      pointer-events: none;
  }
  
  .table-body-wrapper.full-loading {
      min-height: 200px;
  }

  .inner-loader {
      position: absolute;
      top: 50%;
      left: 50%;
      transform: translate(-50%, -50%);
      background: rgba(0, 0, 0, 0.8);
      padding: 16px 24px;
      border-radius: 8px;
      border: 1px solid var(--zen-green);
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 12px;
      z-index: 100;
      color: var(--zen-green);
      font-weight: bold;
      box-shadow: 0 0 20px rgba(0, 255, 136, 0.4);
  }

  .spinner {
      width: 24px;
      height: 24px;
      border: 3px solid rgba(0, 255, 136, 0.2);
      border-top-color: var(--zen-green);
      border-radius: 50%;
      animation: spin 1s linear infinite;
  }

  @keyframes spin {
      to { transform: rotate(360deg); }
  }

  /* Zen Color Palette */
  :root {
    --zen-green: #00ff88;
    --zen-green-dim: rgba(0, 255, 136, 0.2);
    --zen-blue: #00ccff;
  }

  /* Type-specific styling with Neon Glow */
  .type-number {
    color: var(--zen-blue);
    text-align: right;
    text-shadow: 0 0 5px rgba(0, 204, 255, 0.3);
  }
  .type-currency {
    color: var(--zen-green);
    text-align: right;
    font-weight: bold;
    text-shadow: 0 0 8px rgba(0, 255, 136, 0.4);
  }
  .type-boolean {
    color: #ff00ff;
    font-style: italic;
    text-shadow: 0 0 5px rgba(255, 0, 255, 0.3);
  }
  .type-date {
    color: var(--vscode-terminal-ansiYellow);
    opacity: 0.9;
  }
  .type-string {
    color: #e0e0e0;
    text-align: center;
  }
</style>
