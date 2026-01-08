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
        h1 { font-size: 1.25rem; font-weight: 600; }
        h1 span { color: #888; font-weight: 400; margin-left: 0.5rem; }
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

        /* Scrollbar */
        ::-webkit-scrollbar { width: 8px; height: 8px; }
        ::-webkit-scrollbar-track { background: #0a0a0a; }
        ::-webkit-scrollbar-thumb { background: #333; border-radius: 4px; }
        ::-webkit-scrollbar-thumb:hover { background: #444; }

        /* Time column */
        .time { color: #666; font-size: 0.8125rem; }
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>Dvaar Inspector<span id="request-count">0 requests</span></h1>
            <div class="controls">
                <div class="status">
                    <div class="status-dot" id="status-dot"></div>
                    <span id="status-text">Disconnected</span>
                </div>
                <label style="display: flex; align-items: center; gap: 0.5rem; font-size: 0.875rem; color: #888;">
                    <input type="checkbox" id="auto-scroll" checked> Auto-scroll
                </label>
                <button onclick="clearRequests()" class="danger">Clear</button>
            </div>
        </header>

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

    <script>
        let requests = [];
        let ws = null;
        let expandedId = null;

        function connect() {
            const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
            ws = new WebSocket(`${protocol}//${window.location.host}/ws`);

            ws.onopen = () => {
                document.getElementById('status-dot').classList.add('connected');
                document.getElementById('status-text').textContent = 'Connected';
            };

            ws.onclose = () => {
                document.getElementById('status-dot').classList.remove('connected');
                document.getElementById('status-text').textContent = 'Disconnected';
                setTimeout(connect, 2000);
            };

            ws.onerror = () => {
                ws.close();
            };

            ws.onmessage = (event) => {
                const msg = JSON.parse(event.data);
                if (msg.type === 'requests') {
                    requests = msg.data;
                    renderRequests();
                } else if (msg.type === 'request') {
                    requests.push(msg.data);
                    if (requests.length > 50) requests.shift();
                    renderRequests();
                    if (document.getElementById('auto-scroll').checked) {
                        const container = document.getElementById('requests-container');
                        container.scrollTop = container.scrollHeight;
                    }
                } else if (msg.type === 'clear') {
                    requests = [];
                    renderRequests();
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

            countEl.textContent = `${requests.length} request${requests.length !== 1 ? 's' : ''}`;

            if (requests.length === 0) {
                container.innerHTML = '';
                container.appendChild(emptyState);
                emptyState.style.display = 'block';
                return;
            }

            emptyState.style.display = 'none';

            container.innerHTML = requests.map(req => `
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
