<script lang="ts">
  import { onMount } from "svelte";
  import { get_wad_history, sync_wads } from "../../commands";
  import { formatBalance } from "../../utils";

  interface WadHistoryItem {
    id: string;
    wadType: string;
    status: string;
    totalAmountJson: string;
    memo?: string;
    createdAt: number;
    modifiedAt: number;
  }

  let wadHistory: WadHistoryItem[] = $state([]);
  let loading = $state(true);
  let error = $state("");

  onMount(async () => {
    await loadWadHistory();
  });

  async function loadWadHistory() {
    try {
      loading = true;
      error = "";
      
      // Sync WADs first to update their status
      await sync_wads();
      
      const history = await get_wad_history(20);
      
      wadHistory = history || [];
    } catch (err) {
      console.error("Failed to load transfer history:", err);
      error = "Failed to load transfer history: " + String(err);
    } finally {
      loading = false;
    }
  }

  function formatTimestamp(timestamp: number): string {
    return new Date(timestamp * 1000).toLocaleString();
  }

  function formatAmount(amountJson: string): string {
    try {
      console.log("Raw amountJson received:", amountJson);
      const parsed = JSON.parse(amountJson);
      console.log("Parsed data:", parsed);
      
      // The data comes as an array of [unit, amount] pairs
      if (Array.isArray(parsed) && parsed.length > 0) {
        const [unit, amount] = parsed[0];
        
        const formatted = formatBalance({ unit, amount: Number(amount) });
        return `${formatted.amount} ${formatted.asset}`;
      }

      return "0 STRK";
    } catch (e) {
      return "0 STRK";
    }
  }

  function getStatusColor(status: string): string {
    switch (status.toLowerCase()) {
      case "finished": return "#28a745";
      case "pending": return "#ffc107";
      case "failed": return "#dc3545";
      case "cancelled": return "#6c757d";
      default: return "#007bff";
    }
  }

  function getTypeIcon(type: string): string {
    return type.toLowerCase() === "in" ? "📥" : "📤";
  }

  function getTypeDisplay(type: string): string {
    return type.toLowerCase() === "in" ? "IN" : "OUT";
  }
</script>

<div class="history-page">
  <div class="header">
    <h1>Transfer History</h1>
  </div>

  <div class="content">
    {#if loading}
      <div class="loading">
        <div class="spinner"></div>
        <p>Loading transfer history...</p>
      </div>
    {:else if error}
      <div class="error">
        <p>{error}</p>
        <button onclick={loadWadHistory}>Retry</button>
      </div>
    {:else if wadHistory.length === 0}
      <div class="empty">
        <p>No transfer history found</p>
      </div>
    {:else}
      <div class="history-list">
        {#each wadHistory as wad}
          <div class="wad-item">
            <div class="wad-first-line">
              <span class="type-icon">{getTypeIcon(wad.wadType)}</span>
              <span class="type-text">{getTypeDisplay(wad.wadType)}</span>
              <span class="wad-amount">{formatAmount(wad.totalAmountJson)}</span>
              <span class="wad-status" style="color: {getStatusColor(wad.status)}">{wad.status}</span>
            </div>
            <div class="wad-second-line">
              <span class="wad-time">{formatTimestamp(wad.createdAt)}</span>
            </div>
          </div>
        {/each}
      </div>
    {/if}
    
    <div class="refresh-container">
      <button class="refresh-btn" onclick={loadWadHistory} disabled={loading}>
        🔄 Refresh
      </button>
    </div>
  </div>
</div>

<style>
  .history-page {
    display: flex;
    flex-direction: column;
    min-height: 100vh;
    width: 100%;
    background: #ffffff;
    margin: 0;
    padding: 0;
    overflow-x: hidden;
  }

  .header {
    padding: 12px 0;
    border-bottom: 1px solid #eee;
    background: white;
    flex-shrink: 0;
  }

  .header h1 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    text-align: center;
  }

  .content {
    flex: 1;
    padding-bottom: 100px;
  }

  .history-list {
    padding: 0;
    margin: 0;
  }

  .wad-item {
    padding: 12px 16px;
    border-bottom: 1px solid #eee;
    width: 100%;
    box-sizing: border-box;
  }

  .wad-first-line {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 4px;
  }

  .type-icon {
    font-size: 14px;
    flex-shrink: 0;
  }

  .type-text {
    font-size: 14px;
    font-weight: 600;
    text-transform: uppercase;
    flex-shrink: 0;
  }

  .wad-amount {
    flex: 1;
    font-size: 14px;
    font-weight: 500;
    text-align: right;
  }

  .wad-status {
    font-size: 12px;
    font-weight: 500;
    text-transform: uppercase;
    flex-shrink: 0;
    margin-left: 8px;
  }

  .wad-second-line {
    display: flex;
  }

  .wad-time {
    font-size: 12px;
    color: #666;
  }

  .refresh-container {
    position: fixed;
    bottom: 80px;
    left: 0;
    right: 0;
    padding: 12px;
    background: white;
    border-top: 1px solid #eee;
    display: flex;
    justify-content: center;
    flex-shrink: 0;
  }

  .refresh-btn {
    background: #007bff;
    color: white;
    border: none;
    padding: 8px 16px;
    border-radius: 4px;
    font-size: 14px;
    cursor: pointer;
  }

  .refresh-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .loading, .error, .empty {
    padding: 20px;
    text-align: center;
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
  }

  .spinner {
    border: 2px solid #f3f3f3;
    border-top: 2px solid #007bff;
    border-radius: 50%;
    width: 20px;
    height: 20px;
    animation: spin 1s linear infinite;
    margin: 0 auto 10px;
  }

  @keyframes spin {
    0% { transform: rotate(0deg); }
    100% { transform: rotate(360deg); }
  }
</style> 