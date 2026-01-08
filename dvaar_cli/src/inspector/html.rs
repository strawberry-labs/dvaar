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
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, monospace;
            background: #0a0a0a;
            color: #e0e0e0;
            min-height: 100vh;
            font-size: 14px;
        }
        .container { max-width: 1400px; margin: 0 auto; padding: 1rem; }

        /* Header */
        header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 1rem;
            padding-bottom: 1rem;
            border-bottom: 1px solid #222;
        }
        .header-left { display: flex; align-items: center; gap: 1rem; }
        h1 { font-size: 1.25rem; font-weight: 600; }
        .online-badge {
            background: #14532d;
            color: #4ade80;
            font-size: 0.75rem;
            padding: 0.25rem 0.5rem;
            border-radius: 4px;
        }
        .tunnel-selector {
            background: #1a1a1a;
            color: #e0e0e0;
            border: 1px solid #333;
            padding: 0.375rem 0.75rem;
            border-radius: 4px;
            font-size: 0.875rem;
            cursor: pointer;
            min-width: 180px;
        }
        .tunnel-selector:hover { background: #252525; }
        .tunnel-selector option { background: #1a1a1a;
            font-weight: 600;
        }
        .controls { display: flex; gap: 0.5rem; align-items: center; }
        .status {
            display: flex;
            align-items: center;
            gap: 0.5rem;
            font-size: 0.875rem;
            color: #888;
        }
        .status-dot {
            width: 8px;
            height: 8px;
            border-radius: 50%;
            background: #ef4444;
        }
        .status-dot.connected { background: #22c55e; }

        /* Navigation Tabs */
        .nav-tabs {
            display: flex;
            gap: 0;
            margin-bottom: 1.5rem;
            border-bottom: 1px solid #333;
        }
        .nav-tab {
            background: transparent;
            border: none;
            border-bottom: 2px solid transparent;
            color: #888;
            padding: 0.75rem 1.5rem;
            cursor: pointer;
            font-size: 0.9rem;
            font-weight: 500;
            transition: color 0.2s;
        }
        .nav-tab:hover { color: #e0e0e0; }
        .nav-tab.active {
            color: #e0e0e0;
            border-bottom-color: #2563eb;
        }
        .tab-content { display: none; }
        .tab-content.active { display: block; }

        button {
            background: #1a1a1a;
            color: #e0e0e0;
            border: 1px solid #333;
            padding: 0.5rem 1rem;
            border-radius: 4px;
            cursor: pointer;
            font-size: 0.875rem;
        }
        button:hover { background: #252525; }
        button.primary { background: #2563eb; border-color: #2563eb; }
        button.primary:hover { background: #1d4ed8; }
        button.danger { background: #7f1d1d; border-color: #991b1b; }
        button.danger:hover { background: #991b1b; }

        /* Request list */
        .request-list {
            background: #111;
            border: 1px solid #222;
            border-radius: 8px;
            overflow: hidden;
        }
        .request-header {
            display: grid;
            grid-template-columns: 80px 1fr 80px 100px 100px 80px;
            padding: 0.75rem 1rem;
            background: #1a1a1a;
            font-weight: 600;
            font-size: 0.75rem;
            text-transform: uppercase;
            color: #888;
            border-bottom: 1px solid #222;
        }
        .request-row {
            display: grid;
            grid-template-columns: 80px 1fr 80px 100px 100px 80px;
            padding: 0.75rem 1rem;
            border-bottom: 1px solid #1a1a1a;
            cursor: pointer;
            transition: background 0.1s;
        }
        .request-row:hover { background: #1a1a1a; }
        .request-row.expanded { background: #151515; }
        .request-row:last-child { border-bottom: none; }

        /* Method badges */
        .method {
            font-weight: 600;
            font-size: 0.75rem;
            padding: 0.125rem 0.5rem;
            border-radius: 3px;
            display: inline-block;
            text-align: center;
        }
        .method.GET { background: #14532d; color: #4ade80; }
        .method.POST { background: #713f12; color: #fbbf24; }
        .method.PUT { background: #1e3a5f; color: #60a5fa; }
        .method.PATCH { background: #581c87; color: #c084fc; }
        .method.DELETE { background: #7f1d1d; color: #f87171; }
        .method.HEAD { background: #164e63; color: #22d3ee; }
        .method.OPTIONS { background: #374151; color: #9ca3af; }

        /* Status badges */
        .status-badge {
            font-weight: 600;
            font-size: 0.875rem;
        }
        .status-badge.s2xx { color: #4ade80; }
        .status-badge.s3xx { color: #22d3ee; }
        .status-badge.s4xx { color: #fbbf24; }
        .status-badge.s5xx { color: #f87171; }

        /* Timing */
        .timing { color: #888; font-size: 0.875rem; }
        .timing.slow { color: #fbbf24; }
        .timing.very-slow { color: #f87171; }

        /* Size */
        .size { color: #888; font-size: 0.875rem; }

        /* Path */
        .path {
            overflow: hidden;
            text-overflow: ellipsis;
            white-space: nowrap;
            color: #e0e0e0;
        }

        /* Expanded details */
        .request-details {
            display: none;
            padding: 1rem;
            background: #0d0d0d;
            border-bottom: 1px solid #222;
        }
        .request-details.show { display: block; }

        .detail-tabs {
            display: flex;
            gap: 0.5rem;
            margin-bottom: 1rem;
            border-bottom: 1px solid #222;
            padding-bottom: 0.5rem;
        }
        .detail-tab {
            background: none;
            border: none;
            color: #888;
            padding: 0.5rem 1rem;
            cursor: pointer;
            border-radius: 4px 4px 0 0;
        }
        .detail-tab:hover { color: #e0e0e0; }
        .detail-tab.active {
            color: #e0e0e0;
            background: #1a1a1a;
        }

        .detail-content { display: none; }
        .detail-content.show { display: block; }

        .headers-table {
            width: 100%;
            font-size: 0.875rem;
        }
        .headers-table th {
            text-align: left;
            padding: 0.5rem;
            background: #1a1a1a;
            color: #888;
            font-weight: 600;
        }
        .headers-table td {
            padding: 0.5rem;
            border-bottom: 1px solid #1a1a1a;
            word-break: break-all;
        }
        .headers-table td:first-child {
            color: #60a5fa;
            white-space: nowrap;
            width: 200px;
        }

        .body-content {
            background: #0a0a0a;
            border: 1px solid #222;
            border-radius: 4px;
            padding: 1rem;
            overflow-x: auto;
            max-height: 400px;
            overflow-y: auto;
        }
        .body-content pre {
            margin: 0;
            white-space: pre-wrap;
            word-break: break-all;
            font-family: 'Monaco', 'Menlo', monospace;
            font-size: 0.8125rem;
            line-height: 1.5;
        }

        /* Action buttons in row */
        .row-actions {
            display: flex;
            gap: 0.25rem;
        }
        .row-actions button {
            padding: 0.25rem 0.5rem;
            font-size: 0.75rem;
        }

        /* Empty state */
        .empty-state {
            padding: 3rem;
            text-align: center;
            color: #666;
        }
        .empty-state p { margin-bottom: 0.5rem; }

        /* Status Page Styles */
        .status-grid {
            display: grid;
            grid-template-columns: 1fr 1fr;
            gap: 2rem;
        }
        @media (max-width: 900px) {
            .status-grid { grid-template-columns: 1fr; }
        }
        .status-section {
            background: #111;
            border: 1px solid #222;
            border-radius: 8px;
            padding: 1.5rem;
        }
        .status-section h3 {
            font-size: 1.1rem;
            font-weight: 600;
            margin-bottom: 1.25rem;
            color: #e0e0e0;
        }
        .status-section h4 {
            font-size: 0.9rem;
            font-weight: 600;
            margin-top: 1.5rem;
            margin-bottom: 0.75rem;
            color: #888;
        }
        .info-row {
            display: flex;
            justify-content: space-between;
            padding: 0.6rem 0;
            border-bottom: 1px solid #1a1a1a;
        }
        .info-row:last-child { border-bottom: none; }
        .info-label {
            color: #888;
            font-weight: 500;
        }
        .info-value {
            color: #e0e0e0;
            font-family: 'Monaco', 'Menlo', monospace;
            font-size: 0.875rem;
        }
        .info-value a {
            color: #60a5fa;
            text-decoration: none;
        }
        .info-value a:hover { text-decoration: underline; }
        .metrics-grid {
            display: grid;
            grid-template-columns: repeat(2, 1fr);
            gap: 1rem;
        }
        .metric {
            background: #1a1a1a;
            border-radius: 6px;
            padding: 1rem;
            text-align: center;
        }
        .metric-value {
            font-size: 2rem;
            font-weight: 700;
            color: #e0e0e0;
            display: block;
        }
        .metric-label {
            font-size: 0.75rem;
            color: #888;
            text-transform: uppercase;
            letter-spacing: 0.5px;
        }
        .metrics-table {
            width: 100%;
            font-size: 0.875rem;
            border-collapse: collapse;
        }
        .metrics-table th {
            text-align: left;
            padding: 0.5rem;
            color: #666;
            font-weight: 500;
            border-bottom: 1px solid #222;
        }
        .metrics-table td {
            padding: 0.5rem;
            color: #e0e0e0;
            font-family: 'Monaco', 'Menlo', monospace;
        }
        .metrics-table tr:hover td { background: #1a1a1a; }

        /* Scrollbar */
        ::-webkit-scrollbar { width: 8px; height: 8px; }
        ::-webkit-scrollbar-track { background: #0a0a0a; }
        ::-webkit-scrollbar-thumb { background: #333; border-radius: 4px; }
        ::-webkit-scrollbar-thumb:hover { background: #444; }

        /* Time column */
        .time { color: #666; font-size: 0.8125rem; }

        /* Inspect tab header */
        .inspect-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 1rem;
        }
        .inspect-header h2 {
            font-size: 1rem;
            font-weight: 500;
            color: #888;
        }
    </style>
</head>
<body>
    <div class="container">
        <header>
            <div class="header-left">
                <h1>dvaar</h1>
                <span class="online-badge" id="online-badge">online</span>
                <select id="tunnel-selector" class="tunnel-selector" onchange="selectTunnel(this.value)">
                    <option value="">All Tunnels</option>
                </select>
            </div>
            <div class="controls">
                <div class="status">
                    <div class="status-dot" id="status-dot"></div>
                    <span id="status-text">Disconnected</span>
                </div>
            </div>
        </header>

        <!-- Navigation Tabs -->
        <nav class="nav-tabs">
            <button class="nav-tab active" data-tab="inspect" onclick="switchTab('inspect')">Inspect</button>
            <button class="nav-tab" data-tab="status" onclick="switchTab('status')">Status</button>
        </nav>

        <!-- Inspect Tab -->
        <div class="tab-content active" id="tab-inspect">
            <div class="inspect-header">
                <h2 id="request-count">0 requests</h2>
                <div style="display: flex; gap: 0.5rem; align-items: center;">
                    <label style="display: flex; align-items: center; gap: 0.5rem; font-size: 0.875rem; color: #888;">
                        <input type="checkbox" id="auto-scroll" checked> Auto-scroll
                    </label>
                    <button onclick="clearRequests()" class="danger">Clear</button>
                </div>
            </div>

            <div class="request-list" id="request-list">
                <div class="request-header">
                    <div>Method</div>
                    <div>Path</div>
                    <div>Status</div>
                    <div>Time</div>
                    <div>Size</div>
                    <div>Actions</div>
                </div>
                <div id="requests-container">
                    <div class="empty-state" id="empty-state">
                        <p>No requests captured yet</p>
                        <p style="font-size: 0.875rem;">Requests through your tunnel will appear here</p>
                    </div>
                </div>
            </div>
        </div>

        <!-- Status Tab -->
        <div class="tab-content" id="tab-status">
            <div class="status-grid">
                <!-- Tunnel Info Section -->
                <div class="status-section">
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
                        <span class="info-label">Protocol</span>
                        <span class="info-value">HTTP/HTTPS</span>
                    </div>
                    <div class="info-row">
                        <span class="info-label">Inspector</span>
                        <span class="info-value" id="inspector-addr">-</span>
                    </div>
                </div>

                <!-- Metrics Section -->
                <div class="status-section">
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

                    <h4>Request Rate</h4>
                    <table class="metrics-table">
                        <thead>
                            <tr>
                                <th>1 min</th>
                                <th>5 min</th>
                                <th>15 min</th>
                            </tr>
                        </thead>
                        <tbody>
                            <tr>
                                <td id="rate-1m">0.00</td>
                                <td id="rate-5m">0.00</td>
                                <td id="rate-15m">0.00</td>
                            </tr>
                        </tbody>
                    </table>

                    <h4>Response Time Percentiles</h4>
                    <table class="metrics-table">
                        <thead>
                            <tr>
                                <th>p50</th>
                                <th>p90</th>
                                <th>p95</th>
                                <th>p99</th>
                            </tr>
                        </thead>
                        <tbody>
                            <tr>
                                <td id="p50">0ms</td>
                                <td id="p90">0ms</td>
                                <td id="p95">0ms</td>
                                <td id="p99">0ms</td>
                            </tr>
                        </tbody>
                    </table>
                </div>
            </div>
        </div>
    </div>

    <script>
        let requests = [];
        let tunnels = {};
        let selectedTunnelId = null;
        let ws = null;
        let expandedId = null;
        let currentTab = 'inspect';
        let metricsInterval = null;

        function selectTunnel(tunnelId) {
            selectedTunnelId = tunnelId || null;
            renderRequests();
            // Refresh status/metrics for selected tunnel
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
                option.style.color = tunnel.status === 'active' ? '#4ade80' : '#888';
                selector.appendChild(option);
            });
            selector.value = currentValue;
        }

        function getFilteredRequests() {
            if (!selectedTunnelId) return requests;
            return requests.filter(r => r.tunnel_id === selectedTunnelId);
        }

        function switchTab(tab) {
            currentTab = tab;
            document.querySelectorAll('.nav-tab').forEach(t => t.classList.remove('active'));
            document.querySelectorAll('.tab-content').forEach(c => c.classList.remove('active'));
            document.querySelector(`[data-tab="${tab}"]`).classList.add('active');
            document.getElementById(`tab-${tab}`).classList.add('active');

            // Start/stop metrics polling based on active tab
            if (tab === 'status') {
                fetchTunnelInfo();
                fetchMetrics();
                if (!metricsInterval) {
                    metricsInterval = setInterval(fetchMetrics, 2000);
                }
            } else if (metricsInterval) {
                clearInterval(metricsInterval);
                metricsInterval = null;
            }
        }

        async function fetchTunnelInfo() {
            try {
                // If a specific tunnel is selected, use its info from the tunnels cache
                if (selectedTunnelId && tunnels[selectedTunnelId]) {
                    const tunnel = tunnels[selectedTunnelId];
                    const tunnelUrlEl = document.getElementById('tunnel-url');
                    if (tunnel.public_url) {
                        tunnelUrlEl.innerHTML = `<a href="${tunnel.public_url}" target="_blank">${tunnel.public_url}</a>`;
                    } else {
                        tunnelUrlEl.textContent = '-';
                    }
                    document.getElementById('local-addr').textContent = tunnel.local_addr || '-';
                    return;
                }

                // Otherwise fetch the default/aggregate info
                const res = await fetch('/api/info');
                const info = await res.json();

                const tunnelUrlEl = document.getElementById('tunnel-url');
                if (info.public_url) {
                    tunnelUrlEl.innerHTML = `<a href="${info.public_url}" target="_blank">${info.public_url}</a>`;
                } else {
                    tunnelUrlEl.textContent = '-';
                }

                document.getElementById('local-addr').textContent = info.local_addr || '-';
            } catch (e) {
                console.error('Failed to fetch tunnel info:', e);
            }
        }

        async function fetchMetrics() {
            try {
                // Fetch per-tunnel metrics if a tunnel is selected, otherwise aggregate
                const url = selectedTunnelId
                    ? `/api/tunnels/${selectedTunnelId}/metrics`
                    : '/api/metrics';
                const res = await fetch(url);

                if (!res.ok) {
                    // Handle 404 for tunnels that don't exist
                    if (res.status === 404) {
                        document.getElementById('total-requests').textContent = '0';
                        document.getElementById('open-connections').textContent = '0';
                        document.getElementById('rate-1m').textContent = '0.00';
                        document.getElementById('rate-5m').textContent = '0.00';
                        document.getElementById('rate-15m').textContent = '0.00';
                        document.getElementById('p50').textContent = '0ms';
                        document.getElementById('p90').textContent = '0ms';
                        document.getElementById('p95').textContent = '0ms';
                        document.getElementById('p99').textContent = '0ms';
                        return;
                    }
                    throw new Error(`HTTP ${res.status}`);
                }

                const metrics = await res.json();

                document.getElementById('total-requests').textContent = metrics.total_requests;
                document.getElementById('open-connections').textContent = metrics.open_connections;
                document.getElementById('rate-1m').textContent = metrics.requests_per_minute_1m.toFixed(2);
                document.getElementById('rate-5m').textContent = metrics.requests_per_minute_5m.toFixed(2);
                document.getElementById('rate-15m').textContent = metrics.requests_per_minute_15m.toFixed(2);
                document.getElementById('p50').textContent = metrics.p50_duration_ms + 'ms';
                document.getElementById('p90').textContent = metrics.p90_duration_ms + 'ms';
                document.getElementById('p95').textContent = metrics.p95_duration_ms + 'ms';
                document.getElementById('p99').textContent = metrics.p99_duration_ms + 'ms';
            } catch (e) {
                console.error('Failed to fetch metrics:', e);
            }
        }

        function connect() {
            const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
            ws = new WebSocket(`${protocol}//${window.location.host}/ws`);

            ws.onopen = () => {
                document.getElementById('status-dot').classList.add('connected');
                document.getElementById('status-text').textContent = 'Connected';
                document.getElementById('online-badge').textContent = 'online';
                document.getElementById('online-badge').style.background = '#14532d';

                // Set inspector address
                document.getElementById('inspector-addr').textContent = window.location.host;
            };

            ws.onclose = () => {
                document.getElementById('status-dot').classList.remove('connected');
                document.getElementById('status-text').textContent = 'Disconnected';
                document.getElementById('online-badge').textContent = 'offline';
                document.getElementById('online-badge').style.background = '#7f1d1d';
                setTimeout(connect, 2000);
            };

            ws.onerror = () => {
                ws.close();
            };

            ws.onmessage = (event) => {
                const msg = JSON.parse(event.data);
                if (msg.type === 'requests') {
                    requests = msg.data;
                    requestAnimationFrame(() => renderRequests());
                } else if (msg.type === 'request') {
                    // Only show if no tunnel selected or request matches selected tunnel
                    if (!selectedTunnelId || msg.data.tunnel_id === selectedTunnelId) {
                        requests.push(msg.data);
                        if (requests.length > 100) requests.shift();
                        requestAnimationFrame(() => {
                            renderRequests();
                            if (document.getElementById('auto-scroll').checked) {
                                const container = document.getElementById('requests-container');
                                container.scrollTop = container.scrollHeight;
                            }
                        });
                    } else {
                        // Still store it, just don't render immediately
                        requests.push(msg.data);
                        if (requests.length > 100) requests.shift();
                    }
                } else if (msg.type === 'clear') {
                    if (!msg.data || !msg.data.tunnel_id) {
                        requests = [];
                    } else {
                        requests = requests.filter(r => r.tunnel_id !== msg.data.tunnel_id);
                    }
                    requestAnimationFrame(() => renderRequests());
                } else if (msg.type === 'tunnels') {
                    // Initial tunnels list
                    tunnels = {};
                    msg.data.forEach(t => tunnels[t.tunnel_id] = t);
                    updateTunnelSelector();
                } else if (msg.type === 'tunnel_registered') {
                    tunnels[msg.data.tunnel_id] = msg.data;
                    updateTunnelSelector();
                } else if (msg.type === 'tunnel_unregistered') {
                    if (tunnels[msg.data.tunnel_id]) {
                        tunnels[msg.data.tunnel_id].status = 'disconnected';
                    }
                    updateTunnelSelector();
                } else if (msg.type === 'tunnel_status') {
                    if (tunnels[msg.data.tunnel_id]) {
                        tunnels[msg.data.tunnel_id].status = msg.data.status;
                    }
                    updateTunnelSelector();
                } else if (msg.type === 'tunnel_updated') {
                    // Update tunnel info (e.g., when public_url becomes available)
                    tunnels[msg.data.tunnel_id] = msg.data;
                    updateTunnelSelector();
                    // Refresh status page if this tunnel is selected
                    if (currentTab === 'status' && selectedTunnelId === msg.data.tunnel_id) {
                        fetchTunnelInfo();
                    }
                }
            };
        }

        function formatTime(timestamp) {
            const date = new Date(timestamp);
            return date.toLocaleTimeString('en-US', { hour12: false });
        }

        function formatSize(bytes) {
            if (bytes === 0) return '0 B';
            if (bytes < 1024) return bytes + ' B';
            if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
            return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
        }

        function formatDuration(ms) {
            if (ms < 1000) return ms + 'ms';
            return (ms / 1000).toFixed(2) + 's';
        }

        function getStatusClass(status) {
            if (status >= 500) return 's5xx';
            if (status >= 400) return 's4xx';
            if (status >= 300) return 's3xx';
            return 's2xx';
        }

        function getTimingClass(ms) {
            if (ms > 1000) return 'very-slow';
            if (ms > 500) return 'slow';
            return '';
        }

        function decodeBody(base64) {
            try {
                const binary = atob(base64);
                const bytes = new Uint8Array(binary.length);
                for (let i = 0; i < binary.length; i++) {
                    bytes[i] = binary.charCodeAt(i);
                }
                return new TextDecoder().decode(bytes);
            } catch (e) {
                return '[Binary data]';
            }
        }

        function formatBody(base64) {
            const text = decodeBody(base64);
            try {
                const json = JSON.parse(text);
                return JSON.stringify(json, null, 2);
            } catch {
                return text;
            }
        }

        function toggleDetails(id) {
            const details = document.getElementById(`details-${id}`);
            const row = document.getElementById(`row-${id}`);

            if (expandedId === id) {
                details.classList.remove('show');
                row.classList.remove('expanded');
                expandedId = null;
            } else {
                // Close previous
                if (expandedId) {
                    document.getElementById(`details-${expandedId}`)?.classList.remove('show');
                    document.getElementById(`row-${expandedId}`)?.classList.remove('expanded');
                }
                details.classList.add('show');
                row.classList.add('expanded');
                expandedId = id;
                showTab(id, 'request-headers');
            }
        }

        function showTab(id, tab) {
            const details = document.getElementById(`details-${id}`);
            details.querySelectorAll('.detail-tab').forEach(t => t.classList.remove('active'));
            details.querySelectorAll('.detail-content').forEach(c => c.classList.remove('show'));
            details.querySelector(`[data-tab="${tab}"]`).classList.add('active');
            details.querySelector(`#${tab}-${id}`).classList.add('show');
        }

        async function replayRequest(id, event) {
            event.stopPropagation();
            const btn = event.target;
            btn.disabled = true;
            btn.textContent = '...';

            try {
                const res = await fetch(`/api/replay/${id}`, { method: 'POST' });
                const data = await res.json();
                if (data.success) {
                    btn.textContent = data.status;
                    setTimeout(() => btn.textContent = 'Replay', 2000);
                } else {
                    btn.textContent = 'Error';
                    setTimeout(() => btn.textContent = 'Replay', 2000);
                }
            } catch (e) {
                btn.textContent = 'Error';
                setTimeout(() => btn.textContent = 'Replay', 2000);
            }
            btn.disabled = false;
        }

        async function clearRequests() {
            await fetch('/api/clear', { method: 'POST' });
        }

        function renderRequests() {
            const container = document.getElementById('requests-container');
            const emptyState = document.getElementById('empty-state');
            const countEl = document.getElementById('request-count');
            const filtered = getFilteredRequests();

            const tunnelCount = Object.keys(tunnels).length;
            const tunnelLabel = selectedTunnelId ? ' (filtered)' : (tunnelCount > 1 ? ` (${tunnelCount} tunnels)` : '');
            countEl.textContent = `${filtered.length} request${filtered.length !== 1 ? 's' : ''}${tunnelLabel}`;

            if (filtered.length === 0) {
                container.innerHTML = '';
                container.appendChild(emptyState);
                emptyState.style.display = 'block';
                return;
            }

            emptyState.style.display = 'none';

            container.innerHTML = filtered.map(req => `
                <div class="request-row ${expandedId === req.id ? 'expanded' : ''}" id="row-${req.id}" onclick="toggleDetails('${req.id}')">
                    <div><span class="method ${req.method}">${req.method}</span></div>
                    <div class="path" title="${req.path}">${req.path}</div>
                    <div><span class="status-badge ${getStatusClass(req.response_status)}">${req.response_status}</span></div>
                    <div class="timing ${getTimingClass(req.duration_ms)}">${formatDuration(req.duration_ms)}</div>
                    <div class="size">${formatSize(req.size_bytes)}</div>
                    <div class="row-actions">
                        <button onclick="replayRequest('${req.id}', event)" class="primary">Replay</button>
                    </div>
                </div>
                <div class="request-details ${expandedId === req.id ? 'show' : ''}" id="details-${req.id}">
                    <div class="detail-tabs">
                        <button class="detail-tab active" data-tab="request-headers" onclick="showTab('${req.id}', 'request-headers')">Request Headers</button>
                        <button class="detail-tab" data-tab="request-body" onclick="showTab('${req.id}', 'request-body')">Request Body</button>
                        <button class="detail-tab" data-tab="response-headers" onclick="showTab('${req.id}', 'response-headers')">Response Headers</button>
                        <button class="detail-tab" data-tab="response-body" onclick="showTab('${req.id}', 'response-body')">Response Body</button>
                    </div>
                    <div class="detail-content show" id="request-headers-${req.id}">
                        <table class="headers-table">
                            <thead><tr><th>Header</th><th>Value</th></tr></thead>
                            <tbody>
                                ${req.request_headers.map(([k, v]) => `<tr><td>${k}</td><td>${v}</td></tr>`).join('')}
                            </tbody>
                        </table>
                    </div>
                    <div class="detail-content" id="request-body-${req.id}">
                        <div class="body-content"><pre>${formatBody(req.request_body) || '(empty)'}</pre></div>
                    </div>
                    <div class="detail-content" id="response-headers-${req.id}">
                        <table class="headers-table">
                            <thead><tr><th>Header</th><th>Value</th></tr></thead>
                            <tbody>
                                ${req.response_headers.map(([k, v]) => `<tr><td>${k}</td><td>${v}</td></tr>`).join('')}
                            </tbody>
                        </table>
                    </div>
                    <div class="detail-content" id="response-body-${req.id}">
                        <div class="body-content"><pre>${formatBody(req.response_body) || '(empty)'}</pre></div>
                    </div>
                </div>
            `).join('');
        }

        connect();
    </script>
</body>
</html>"#;
