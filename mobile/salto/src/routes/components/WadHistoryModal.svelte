<script lang="ts">
  import { onMount } from "svelte";
  import { get_wad_history } from "../../commands";
  import { formatBalance } from "../../utils";

  interface Props {
    onClose: () => void;
  }

  let { onClose }: Props = $props();

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
        console.log("Unit:", unit, "Amount:", amount);
        
        const formatted = formatBalance({ unit, amount: Number(amount) });
        console.log("Formatted result:", formatted);
        return `${formatted.amount} ${formatted.asset}`;
      }

      console.error("Invalid amount format:", parsed);
      return "0 STRK";
    } catch (e) {
      console.error("Error parsing amount:", e);
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

<div class="modal-overlay" 
     role="dialog" 
     aria-modal="true"
     tabindex="-1">
  <button class="modal-overlay-button"
     onclick={onClose} 
     onkeydown={(e) => e.key === 'Escape' && onClose()}>
    <span class="sr-only">Close modal</span>
  </button>
  <div class="modal-content">
    <div class="modal-header">
      <h2>Transfer History</h2>
      <button class="close-btn" onclick={onClose}>✕</button>
    </div>

    <div class="modal-body">
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
              <div class="wad-line">
                <span class="type-icon">{getTypeIcon(wad.wadType)}</span>
                <span class="wad-amount">{formatAmount(wad.totalAmountJson)}</span>
                <span class="wad-status" style="color: {getStatusColor(wad.status)}">{wad.status}</span>
              </div>
              <div class="wad-time">{formatTimestamp(wad.createdAt)}</div>
            </div>
          {/each}
        </div>
      {/if}
    </div>

    <div class="modal-footer">
      <button onclick={loadWadHistory} disabled={loading}>
        🔄 Refresh
      </button>
    </div>
  </div>
</div>

<style>
  .modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-color: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .modal-overlay-button {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    width: 100%;
    height: 100%;
    background: none;
    border: none;
    cursor: pointer;
    padding: 0;
    margin: 0;
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }

  .modal-content {
    background: white;
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .modal-header {
    padding: 12px 16px;
    border-bottom: 1px solid #eee;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .modal-header h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
  }

  .close-btn {
    background: none;
    border: none;
    font-size: 20px;
    color: #666;
    cursor: pointer;
    padding: 4px;
  }

  .modal-body {
    flex: 1;
    overflow-y: auto;
    padding: 0;
  }

  .history-list {
    display: flex;
    flex-direction: column;
  }

  .wad-item {
    padding: 12px 16px;
    border-bottom: 1px solid #eee;
  }

  .wad-line {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 4px;
  }

  .type-icon {
    font-size: 14px;
  }

  .wad-amount {
    flex: 1;
    font-size: 14px;
    font-weight: 500;
  }

  .wad-status {
    font-size: 12px;
    font-weight: 500;
    text-transform: uppercase;
  }

  .wad-time {
    font-size: 12px;
    color: #666;
  }

  .modal-footer {
    padding: 12px 16px;
    border-top: 1px solid #eee;
    display: flex;
    justify-content: center;
  }

  .modal-footer button {
    background: #007bff;
    color: white;
    border: none;
    padding: 8px 16px;
    border-radius: 4px;
    font-size: 14px;
    cursor: pointer;
  }

  .modal-footer button:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .loading, .error, .empty {
    padding: 20px;
    text-align: center;
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