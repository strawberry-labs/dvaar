//! Embedded HTML dashboard for the inspector

pub const INSPECTOR_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Dvaar Inspector</title>
    <style>
        * { box-sizing: border-box; margin: 0; padding: 0; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #fafafa;
            color: #333;
            min-height: 100vh;
            font-size: 14px;
        }

        /* Header */
        header {
            background: #fff;
            border-bottom: 1px solid #e0e0e0;
            padding: 0.75rem 1rem;
            display: flex;
            justify-content: space-between;
            align-items: center;
            position: sticky;
            top: 0;
            z-index: 100;
        }
        .header-left { display: flex; align-items: center; gap: 1rem; }
        h1 { font-size: 1.1rem; font-weight: 600; color: #333; }
        .status-badge {
            font-size: 0.75rem;
            padding: 0.2rem 0.5rem;
            border-radius: 3px;
            font-weight: 500;
        }
        .status-badge.online { background: #dcfce7; color: #166534; }
        .status-badge.offline { background: #fee2e2; color: #991b1b; }

        .tunnel-selector {
            background: #fff;
            border: 1px solid #d0d0d0;
            padding: 0.4rem 0.75rem;
            border-radius: 4px;
            font-size: 0.875rem;
            cursor: pointer;
            min-width: 160px;
        }

        .controls { display: flex; gap: 0.5rem; align-items: center; }
        .ws-status {
            display: flex;
            align-items: center;
            gap: 0.4rem;
            font-size: 0.8rem;
            color: #666;
        }
        .ws-dot {
            width: 8px;
            height: 8px;
            border-radius: 50%;
            background: #dc2626;
        }
        .ws-dot.connected { background: #16a34a; }

        /* Navigation Tabs */
        .nav-tabs {
            background: #fff;
            border-bottom: 1px solid #e0e0e0;
            display: flex;
            padding: 0 1rem;
        }
        .nav-tab {
            background: transparent;
            border: none;
            border-bottom: 2px solid transparent;
            color: #666;
            padding: 0.75rem 1.25rem;
            cursor: pointer;
            font-size: 0.875rem;
            font-weight: 500;
        }
        .nav-tab:hover { color: #333; }
        .nav-tab.active { color: #333; border-bottom-color: #2563eb; }

        .tab-content { display: none; }
        .tab-content.active { display: block; }

        /* Two-column layout for Inspect tab */
        .inspect-container {
            display: flex;
            height: calc(100vh - 105px);
        }

        /* Left panel - request list */
        .request-list-panel {
            width: 45%;
            min-width: 350px;
            max-width: 600px;
            border-right: 1px solid #e0e0e0;
            display: flex;
            flex-direction: column;
            background: #fff;
        }

        .list-header {
            padding: 0.75rem 1rem;
            border-bottom: 1px solid #e0e0e0;
            display: flex;
            justify-content: space-between;
            align-items: center;
            background: #fafafa;
        }
        .list-header h2 {
            font-size: 0.9rem;
            font-weight: 500;
            color: #666;
        }

        .filter-input {
            width: 100%;
            padding: 0.5rem 0.75rem;
            border: 1px solid #e0e0e0;
            border-radius: 4px;
            font-size: 0.875rem;
            margin: 0.5rem 1rem;
        }
        .filter-input:focus {
            outline: none;
            border-color: #2563eb;
        }

        .request-list {
            flex: 1;
            overflow-y: auto;
        }

        .request-item {
            padding: 0.6rem 1rem;
            border-bottom: 1px solid #f0f0f0;
            cursor: pointer;
            display: flex;
            align-items: center;
            gap: 0.75rem;
        }
        .request-item:hover { background: #f5f5f5; }
        .request-item.selected { background: #eff6ff; border-left: 3px solid #2563eb; }

        .method {
            font-weight: 600;
            font-size: 0.75rem;
            padding: 0.15rem 0.4rem;
            border-radius: 3px;
            min-width: 50px;
            text-align: center;
        }
        .method.GET { background: #dcfce7; color: #166534; }
        .method.POST { background: #fef3c7; color: #92400e; }
        .method.PUT { background: #dbeafe; color: #1e40af; }
        .method.PATCH { background: #f3e8ff; color: #7c3aed; }
        .method.DELETE { background: #fee2e2; color: #991b1b; }
        .method.HEAD { background: #e0f2fe; color: #0369a1; }
        .method.OPTIONS { background: #f3f4f6; color: #4b5563; }

        .request-path {
            flex: 1;
            overflow: hidden;
            text-overflow: ellipsis;
            white-space: nowrap;
            font-size: 0.875rem;
            color: #333;
        }

        .request-meta {
            display: flex;
            gap: 0.5rem;
            align-items: center;
            font-size: 0.8rem;
            color: #888;
        }
        .request-status {
            font-weight: 600;
        }
        .request-status.s2xx { color: #16a34a; }
        .request-status.s3xx { color: #0891b2; }
        .request-status.s4xx { color: #ca8a04; }
        .request-status.s5xx { color: #dc2626; }

        /* Right panel - request details */
        .detail-panel {
            flex: 1;
            overflow-y: auto;
            background: #fff;
        }

        .detail-empty {
            display: flex;
            align-items: center;
            justify-content: center;
            height: 100%;
            color: #999;
            font-size: 0.9rem;
        }

        .detail-header {
            padding: 1rem 1.5rem;
            border-bottom: 1px solid #e0e0e0;
            display: flex;
            justify-content: space-between;
            align-items: flex-start;
            background: #fafafa;
        }

        .detail-title {
            display: flex;
            flex-direction: column;
            gap: 0.5rem;
        }
        .detail-title h3 {
            font-size: 1rem;
            font-weight: 600;
            color: #333;
        }
        .detail-title .meta {
            font-size: 0.8rem;
            color: #666;
            display: flex;
            gap: 1rem;
        }

        .detail-actions {
            display: flex;
            gap: 0.5rem;
        }

        button {
            background: #fff;
            color: #333;
            border: 1px solid #d0d0d0;
            padding: 0.4rem 0.75rem;
            border-radius: 4px;
            cursor: pointer;
            font-size: 0.8rem;
            font-weight: 500;
        }
        button:hover { background: #f5f5f5; }
        button.primary { background: #2563eb; color: #fff; border-color: #2563eb; }
        button.primary:hover { background: #1d4ed8; }
        button.danger { background: #fff; color: #dc2626; border-color: #dc2626; }
        button.danger:hover { background: #fef2f2; }

        /* Request/Response sections */
        .section {
            border-bottom: 1px solid #e0e0e0;
        }
        .section:last-child { border-bottom: none; }

        .section-header {
            padding: 0.75rem 1.5rem;
            font-weight: 600;
            font-size: 0.9rem;
            display: flex;
            justify-content: space-between;
            align-items: center;
        }
        .section-header.request-section {
            background: #f8fafc;
            color: #1e40af;
        }
        .section-header.response-section {
            background: #f0fdf4;
            color: #166534;
        }

        .section-tabs {
            display: flex;
            gap: 0;
            padding: 0 1.5rem;
            border-bottom: 1px solid #e0e0e0;
            background: #fff;
        }
        .section-tab {
            background: transparent;
            border: none;
            border-bottom: 2px solid transparent;
            color: #666;
            padding: 0.6rem 1rem;
            cursor: pointer;
            font-size: 0.8rem;
            font-weight: 500;
        }
        .section-tab:hover { color: #333; }
        .section-tab.active { color: #333; border-bottom-color: #333; }

        .section-content {
            padding: 1rem 1.5rem;
        }

        /* Headers table */
        .headers-table {
            width: 100%;
            font-size: 0.8rem;
            border-collapse: collapse;
        }
        .headers-table th {
            text-align: left;
            padding: 0.4rem 0.5rem;
            background: #f5f5f5;
            color: #666;
            font-weight: 600;
            border-bottom: 1px solid #e0e0e0;
        }
        .headers-table td {
            padding: 0.4rem 0.5rem;
            border-bottom: 1px solid #f0f0f0;
            word-break: break-all;
        }
        .headers-table td:first-child {
            color: #2563eb;
            font-weight: 500;
            white-space: nowrap;
            width: 180px;
        }

        /* Body content */
        .body-info {
            font-size: 0.8rem;
            color: #666;
            margin-bottom: 0.75rem;
        }
        .body-content {
            background: #f8f8f8;
            border: 1px solid #e0e0e0;
            border-radius: 4px;
            padding: 0.75rem;
            overflow-x: auto;
            max-height: 300px;
            overflow-y: auto;
        }
        .body-content pre {
            margin: 0;
            white-space: pre-wrap;
            word-break: break-all;
            font-family: 'Monaco', 'Menlo', 'Consolas', monospace;
            font-size: 0.8rem;
            line-height: 1.5;
        }

        /* JSON highlighting */
        .json-key { color: #881391; }
        .json-string { color: #c41a16; }
        .json-number { color: #1c00cf; }
        .json-boolean { color: #0d22aa; }
        .json-null { color: #808080; }

        /* Status Page */
        .status-container {
            padding: 1.5rem;
            max-width: 1000px;
            margin: 0 auto;
        }
        .status-grid {
            display: grid;
            grid-template-columns: 1fr 1fr;
            gap: 1.5rem;
        }
        @media (max-width: 768px) {
            .status-grid { grid-template-columns: 1fr; }
            .inspect-container { flex-direction: column; }
            .request-list-panel { width: 100%; max-width: none; height: 40vh; min-width: auto; }
            .detail-panel { height: 60vh; }
        }

        .status-card {
            background: #fff;
            border: 1px solid #e0e0e0;
            border-radius: 8px;
            padding: 1.25rem;
        }
        .status-card h3 {
            font-size: 0.9rem;
            font-weight: 600;
            margin-bottom: 1rem;
            color: #333;
        }
        .info-row {
            display: flex;
            justify-content: space-between;
            padding: 0.5rem 0;
            border-bottom: 1px solid #f0f0f0;
        }
        .info-row:last-child { border-bottom: none; }
        .info-label { color: #666; font-weight: 500; }
        .info-value { color: #333; font-family: 'Monaco', monospace; font-size: 0.85rem; }
        .info-value a { color: #2563eb; text-decoration: none; }
        .info-value a:hover { text-decoration: underline; }

        .metrics-grid {
            display: grid;
            grid-template-columns: 1fr 1fr;
            gap: 1rem;
            margin-bottom: 1rem;
        }
        .metric {
            background: #f8f8f8;
            border-radius: 6px;
            padding: 1rem;
            text-align: center;
        }
        .metric-value { font-size: 1.75rem; font-weight: 700; color: #333; }
        .metric-label { font-size: 0.75rem; color: #666; text-transform: uppercase; }

        .metrics-table {
            width: 100%;
            font-size: 0.85rem;
            border-collapse: collapse;
        }
        .metrics-table th {
            text-align: left;
            padding: 0.4rem;
            color: #666;
            font-weight: 500;
            border-bottom: 1px solid #e0e0e0;
        }
        .metrics-table td {
            padding: 0.4rem;
            color: #333;
            font-family: 'Monaco', monospace;
        }

        /* Empty state */
        .empty-state {
            padding: 3rem;
            text-align: center;
            color: #999;
        }

        /* Scrollbar */
        ::-webkit-scrollbar { width: 6px; height: 6px; }
        ::-webkit-scrollbar-track { background: #f0f0f0; }
        ::-webkit-scrollbar-thumb { background: #ccc; border-radius: 3px; }
        ::-webkit-scrollbar-thumb:hover { background: #bbb; }
    </style>
</head>
<body>
    <header>
        <div class="header-left">
            <h1>dvaar</h1>
            <span class="status-badge online" id="status-badge">online</span>
            <select id="tunnel-selector" class="tunnel-selector" onchange="selectTunnel(this.value)">
                <option value="">All Tunnels</option>
            </select>
        </div>
        <div class="controls">
            <div class="ws-status">
                <div class="ws-dot" id="ws-dot"></div>
                <span id="ws-text">Disconnected</span>
            </div>
        </div>
    </header>

    <nav class="nav-tabs">
        <button class="nav-tab active" data-tab="inspect" onclick="switchTab('inspect')">Inspect</button>
        <button class="nav-tab" data-tab="status" onclick="switchTab('status')">Status</button>
    </nav>

    <!-- Inspect Tab - Two Column Layout -->
    <div class="tab-content active" id="tab-inspect">
        <div class="inspect-container">
            <!-- Left: Request List -->
            <div class="request-list-panel">
                <input type="text" class="filter-input" placeholder="Filter by path, method, or status..." id="filter-input" oninput="filterRequests()">
                <div class="list-header">
                    <h2 id="request-count">All Requests</h2>
                    <button onclick="clearRequests()" class="danger">Clear Requests</button>
                </div>
                <div class="request-list" id="request-list">
                    <div class="empty-state" id="empty-state">
                        <p>No requests captured yet</p>
                        <p style="font-size: 0.8rem; margin-top: 0.5rem;">Requests through your tunnel will appear here</p>
                    </div>
                </div>
            </div>

            <!-- Right: Request Details -->
            <div class="detail-panel" id="detail-panel">
                <div class="detail-empty" id="detail-empty">
                    Select a request to view details
                </div>
                <div id="detail-content" style="display: none;"></div>
            </div>
        </div>
    </div>

    <!-- Status Tab -->
    <div class="tab-content" id="tab-status">
        <div class="status-container">
            <div class="status-grid">
                <div class="status-card">
                    <h3>Tunnel Info</h3>
                    <div class="info-row">
                        <span class="info-label">Public URL</span>
                        <span class="info-value" id="tunnel-url">-</span>
                    </div>
                    <div class="info-row">
                        <span class="info-label">Local Address</span>
                        <span class="info-value" id="local-addr">-</span>
                    </div>
                    <div class="info-row">
                        <span class="info-label">Inspector</span>
                        <span class="info-value" id="inspector-addr">-</span>
                    </div>
                </div>

                <div class="status-card">
                    <h3>Metrics</h3>
                    <div class="metrics-grid">
                        <div class="metric">
                            <span class="metric-value" id="total-requests">0</span>
                            <span class="metric-label">Total Requests</span>
                        </div>
                        <div class="metric">
                            <span class="metric-value" id="open-connections">0</span>
                            <span class="metric-label">Open Connections</span>
                        </div>
                    </div>
                    <h4 style="font-size: 0.85rem; color: #666; margin: 1rem 0 0.5rem;">Request Rate (req/min)</h4>
                    <table class="metrics-table">
                        <tr><th>1 min</th><th>5 min</th><th>15 min</th></tr>
                        <tr><td id="rate-1m">0.00</td><td id="rate-5m">0.00</td><td id="rate-15m">0.00</td></tr>
                    </table>
                    <h4 style="font-size: 0.85rem; color: #666; margin: 1rem 0 0.5rem;">Response Time (ms)</h4>
                    <table class="metrics-table">
                        <tr><th>p50</th><th>p90</th><th>p95</th><th>p99</th></tr>
                        <tr><td id="p50">0</td><td id="p90">0</td><td id="p95">0</td><td id="p99">0</td></tr>
                    </table>
                </div>
            </div>
        </div>
    </div>

    <script>
        let requests = [];
        let tunnels = {};
        let selectedTunnelId = null;
        let selectedRequestId = null;
        let ws = null;
        let currentTab = 'inspect';
        let metricsInterval = null;
        let filterText = '';

        function selectTunnel(tunnelId) {
            selectedTunnelId = tunnelId || null;
            renderRequests();
            if (currentTab === 'status') {
                fetchTunnelInfo();
                fetchMetrics();
            }
        }

        function updateTunnelSelector() {
            const selector = document.getElementById('tunnel-selector');
            const currentValue = selector.value;
            selector.innerHTML = '<option value="">All Tunnels</option>';
            Object.values(tunnels).forEach(tunnel => {
                const option = document.createElement('option');
                option.value = tunnel.tunnel_id;
                const status = tunnel.status === 'active' ? '●' : '○';
                const name = tunnel.subdomain || tunnel.tunnel_id.slice(0, 8);
                option.textContent = `${status} ${name}`;
                selector.appendChild(option);
            });
            selector.value = currentValue;
        }

        function getFilteredRequests() {
            let filtered = selectedTunnelId
                ? requests.filter(r => r.tunnel_id === selectedTunnelId)
                : requests;

            if (filterText) {
                const search = filterText.toLowerCase();
                filtered = filtered.filter(r =>
                    r.path.toLowerCase().includes(search) ||
                    r.method.toLowerCase().includes(search) ||
                    r.response_status.toString().includes(search)
                );
            }
            return filtered;
        }

        function filterRequests() {
            filterText = document.getElementById('filter-input').value;
            renderRequests();
        }

        function switchTab(tab) {
            currentTab = tab;
            document.querySelectorAll('.nav-tab').forEach(t => t.classList.remove('active'));
            document.querySelectorAll('.tab-content').forEach(c => c.classList.remove('active'));
            document.querySelector(`[data-tab="${tab}"]`).classList.add('active');
            document.getElementById(`tab-${tab}`).classList.add('active');

            if (tab === 'status') {
                fetchTunnelInfo();
                fetchMetrics();
                if (!metricsInterval) metricsInterval = setInterval(fetchMetrics, 2000);
            } else if (metricsInterval) {
                clearInterval(metricsInterval);
                metricsInterval = null;
            }
        }

        async function fetchTunnelInfo() {
            try {
                if (selectedTunnelId && tunnels[selectedTunnelId]) {
                    const tunnel = tunnels[selectedTunnelId];
                    const urlEl = document.getElementById('tunnel-url');
                    urlEl.innerHTML = tunnel.public_url
                        ? `<a href="${tunnel.public_url}" target="_blank">${tunnel.public_url}</a>`
                        : '-';
                    document.getElementById('local-addr').textContent = tunnel.local_addr || '-';
                    return;
                }
                const res = await fetch('/api/info');
                const info = await res.json();
                const urlEl = document.getElementById('tunnel-url');
                urlEl.innerHTML = info.public_url
                    ? `<a href="${info.public_url}" target="_blank">${info.public_url}</a>`
                    : '-';
                document.getElementById('local-addr').textContent = info.local_addr || '-';
            } catch (e) { console.error('Failed to fetch info:', e); }
        }

        async function fetchMetrics() {
            try {
                const url = selectedTunnelId ? `/api/tunnels/${selectedTunnelId}/metrics` : '/api/metrics';
                const res = await fetch(url);
                if (!res.ok) throw new Error(`HTTP ${res.status}`);
                const m = await res.json();
                document.getElementById('total-requests').textContent = m.total_requests;
                document.getElementById('open-connections').textContent = m.open_connections;
                document.getElementById('rate-1m').textContent = m.requests_per_minute_1m.toFixed(2);
                document.getElementById('rate-5m').textContent = m.requests_per_minute_5m.toFixed(2);
                document.getElementById('rate-15m').textContent = m.requests_per_minute_15m.toFixed(2);
                document.getElementById('p50').textContent = m.p50_duration_ms;
                document.getElementById('p90').textContent = m.p90_duration_ms;
                document.getElementById('p95').textContent = m.p95_duration_ms;
                document.getElementById('p99').textContent = m.p99_duration_ms;
            } catch (e) { console.error('Failed to fetch metrics:', e); }
        }

        function connect() {
            const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
            ws = new WebSocket(`${protocol}//${window.location.host}/ws`);

            ws.onopen = () => {
                document.getElementById('ws-dot').classList.add('connected');
                document.getElementById('ws-text').textContent = 'Connected';
                document.getElementById('status-badge').textContent = 'online';
                document.getElementById('status-badge').className = 'status-badge online';
                document.getElementById('inspector-addr').textContent = window.location.host;
            };

            ws.onclose = () => {
                document.getElementById('ws-dot').classList.remove('connected');
                document.getElementById('ws-text').textContent = 'Disconnected';
                document.getElementById('status-badge').textContent = 'offline';
                document.getElementById('status-badge').className = 'status-badge offline';
                setTimeout(connect, 2000);
            };

            ws.onerror = () => ws.close();

            ws.onmessage = (event) => {
                const msg = JSON.parse(event.data);
                if (msg.type === 'requests') {
                    requests = msg.data;
                    renderRequests();
                } else if (msg.type === 'request') {
                    requests.push(msg.data);
                    if (requests.length > 200) requests.shift();
                    renderRequests();
                } else if (msg.type === 'clear') {
                    if (!msg.data?.tunnel_id) requests = [];
                    else requests = requests.filter(r => r.tunnel_id !== msg.data.tunnel_id);
                    selectedRequestId = null;
                    renderRequests();
                    renderDetails();
                } else if (msg.type === 'tunnels') {
                    tunnels = {};
                    msg.data.forEach(t => tunnels[t.tunnel_id] = t);
                    updateTunnelSelector();
                } else if (msg.type === 'tunnel_registered') {
                    tunnels[msg.data.tunnel_id] = msg.data;
                    updateTunnelSelector();
                } else if (msg.type === 'tunnel_unregistered') {
                    if (tunnels[msg.data.tunnel_id]) tunnels[msg.data.tunnel_id].status = 'disconnected';
                    updateTunnelSelector();
                } else if (msg.type === 'tunnel_status') {
                    if (tunnels[msg.data.tunnel_id]) tunnels[msg.data.tunnel_id].status = msg.data.status;
                    updateTunnelSelector();
                } else if (msg.type === 'tunnel_updated') {
                    tunnels[msg.data.tunnel_id] = msg.data;
                    updateTunnelSelector();
                    if (currentTab === 'status' && selectedTunnelId === msg.data.tunnel_id) fetchTunnelInfo();
                }
            };
        }

        function formatTime(timestamp) {
            return new Date(timestamp).toLocaleTimeString('en-US', { hour12: false });
        }

        function formatTimeAgo(timestamp) {
            const seconds = Math.floor((Date.now() - new Date(timestamp)) / 1000);
            if (seconds < 60) return `${seconds}s ago`;
            const minutes = Math.floor(seconds / 60);
            if (minutes < 60) return `${minutes}m ago`;
            const hours = Math.floor(minutes / 60);
            return `${hours}h ago`;
        }

        function formatSize(bytes) {
            if (bytes === 0) return '0 B';
            if (bytes < 1024) return bytes + ' B';
            if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
            return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
        }

        function formatDuration(ms) {
            if (ms < 1000) return ms.toFixed(2) + 'ms';
            return (ms / 1000).toFixed(2) + 's';
        }

        function getStatusClass(status) {
            if (status >= 500) return 's5xx';
            if (status >= 400) return 's4xx';
            if (status >= 300) return 's3xx';
            return 's2xx';
        }

        function decodeBody(base64) {
            try {
                const binary = atob(base64);
                const bytes = new Uint8Array(binary.length);
                for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
                return new TextDecoder().decode(bytes);
            } catch (e) { return '[Binary data]'; }
        }

        function formatJson(text) {
            try {
                return JSON.stringify(JSON.parse(text), null, 2);
            } catch { return text; }
        }

        function selectRequest(id) {
            selectedRequestId = id;
            renderRequests();
            renderDetails();
        }

        async function replayRequest(id, event) {
            event.stopPropagation();
            const btn = event.target;
            btn.disabled = true;
            btn.textContent = '...';
            try {
                const res = await fetch(`/api/replay/${id}`, { method: 'POST' });
                const data = await res.json();
                btn.textContent = data.success ? data.status : 'Error';
                setTimeout(() => { btn.textContent = 'Replay'; btn.disabled = false; }, 2000);
            } catch (e) {
                btn.textContent = 'Error';
                setTimeout(() => { btn.textContent = 'Replay'; btn.disabled = false; }, 2000);
            }
        }

        async function clearRequests() {
            await fetch('/api/clear', { method: 'POST' });
        }

        function renderRequests() {
            const container = document.getElementById('request-list');
            const emptyState = document.getElementById('empty-state');
            const countEl = document.getElementById('request-count');
            const filtered = getFilteredRequests();

            const tunnelCount = Object.keys(tunnels).length;
            const suffix = selectedTunnelId ? ' (filtered)' : (tunnelCount > 1 ? ` (${tunnelCount} tunnels)` : '');
            countEl.textContent = `${filtered.length} request${filtered.length !== 1 ? 's' : ''}${suffix}`;

            if (filtered.length === 0) {
                container.innerHTML = '';
                container.appendChild(emptyState);
                emptyState.style.display = 'block';
                return;
            }

            emptyState.style.display = 'none';
            container.innerHTML = filtered.slice().reverse().map(req => `
                <div class="request-item ${selectedRequestId === req.id ? 'selected' : ''}" onclick="selectRequest('${req.id}')">
                    <span class="method ${req.method}">${req.method}</span>
                    <span class="request-path" title="${req.path}">${req.path}</span>
                    <div class="request-meta">
                        <span class="request-status ${getStatusClass(req.response_status)}">${req.response_status}</span>
                        <span>${formatDuration(req.duration_ms)}</span>
                    </div>
                </div>
            `).join('');
        }

        function renderDetails() {
            const empty = document.getElementById('detail-empty');
            const content = document.getElementById('detail-content');

            if (!selectedRequestId) {
                empty.style.display = 'flex';
                content.style.display = 'none';
                content.innerHTML = '';  // Clear any stale content
                return;
            }

            const req = requests.find(r => r.id === selectedRequestId);
            if (!req) {
                empty.style.display = 'flex';
                content.style.display = 'none';
                content.innerHTML = '';  // Clear any stale content
                selectedRequestId = null;  // Reset invalid selection
                renderRequests();  // Update request list to remove selection highlight
                return;
            }

            empty.style.display = 'none';
            content.style.display = 'block';

            const reqBody = decodeBody(req.request_body);
            const resBody = decodeBody(req.response_body);
            const contentType = req.response_headers.find(h => h[0].toLowerCase() === 'content-type')?.[1] || '';

            content.innerHTML = `
                <div class="detail-header">
                    <div class="detail-title">
                        <h3><span class="method ${req.method}">${req.method}</span> ${req.path}</h3>
                        <div class="meta">
                            <span>${formatTimeAgo(req.timestamp)}</span>
                            <span>Duration ${formatDuration(req.duration_ms)}</span>
                            <span>${formatSize(req.size_bytes)}</span>
                        </div>
                    </div>
                    <div class="detail-actions">
                        <button class="primary" onclick="replayRequest('${req.id}', event)">Replay</button>
                    </div>
                </div>

                <!-- Request Section -->
                <div class="section">
                    <div class="section-header request-section">
                        <span>${req.method} ${req.path}</span>
                    </div>
                    <div class="section-tabs" id="req-tabs">
                        <button class="section-tab active" onclick="showSection('req', 'headers')">Headers</button>
                        <button class="section-tab" onclick="showSection('req', 'body')">Body</button>
                    </div>
                    <div class="section-content" id="req-headers">
                        <table class="headers-table">
                            <thead><tr><th>Header</th><th>Value</th></tr></thead>
                            <tbody>${req.request_headers.map(([k, v]) => `<tr><td>${escapeHtml(k)}</td><td>${escapeHtml(v)}</td></tr>`).join('')}</tbody>
                        </table>
                    </div>
                    <div class="section-content" id="req-body" style="display: none;">
                        <div class="body-info">${reqBody ? formatSize(reqBody.length) : '(empty)'}</div>
                        <div class="body-content"><pre>${escapeHtml(formatJson(reqBody)) || '(empty)'}</pre></div>
                    </div>
                </div>

                <!-- Response Section -->
                <div class="section">
                    <div class="section-header response-section">
                        <span>${req.response_status} ${getStatusText(req.response_status)}</span>
                    </div>
                    <div class="section-tabs" id="res-tabs">
                        <button class="section-tab active" onclick="showSection('res', 'headers')">Headers</button>
                        <button class="section-tab" onclick="showSection('res', 'body')">Body</button>
                    </div>
                    <div class="section-content" id="res-headers">
                        <table class="headers-table">
                            <thead><tr><th>Header</th><th>Value</th></tr></thead>
                            <tbody>${req.response_headers.map(([k, v]) => `<tr><td>${escapeHtml(k)}</td><td>${escapeHtml(v)}</td></tr>`).join('')}</tbody>
                        </table>
                    </div>
                    <div class="section-content" id="res-body" style="display: none;">
                        <div class="body-info">${contentType ? contentType + ' - ' : ''}${resBody ? formatSize(resBody.length) : '(empty)'}</div>
                        <div class="body-content"><pre>${escapeHtml(formatJson(resBody)) || '(empty)'}</pre></div>
                    </div>
                </div>
            `;
        }

        function showSection(prefix, tab) {
            const tabs = document.getElementById(`${prefix}-tabs`);
            tabs.querySelectorAll('.section-tab').forEach(t => t.classList.remove('active'));
            tabs.querySelector(`[onclick="showSection('${prefix}', '${tab}')"]`).classList.add('active');

            document.getElementById(`${prefix}-headers`).style.display = tab === 'headers' ? 'block' : 'none';
            document.getElementById(`${prefix}-body`).style.display = tab === 'body' ? 'block' : 'none';
        }

        function getStatusText(status) {
            const texts = {
                200: 'OK', 201: 'Created', 204: 'No Content',
                301: 'Moved Permanently', 302: 'Found', 304: 'Not Modified',
                400: 'Bad Request', 401: 'Unauthorized', 403: 'Forbidden', 404: 'Not Found',
                500: 'Internal Server Error', 502: 'Bad Gateway', 503: 'Service Unavailable'
            };
            return texts[status] || '';
        }

        function escapeHtml(text) {
            if (!text) return '';
            return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
        }

        connect();
    </script>
</body>
</html>"#;
