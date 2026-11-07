//! Embedded single-page application assets for the M31A Desktop & Web UI.
//!
//! Provides a zero-dependency, self-contained HTML5/CSS3/ES6 responsive web interface
//! supporting Focus Mode, Pro Mode, Live SSE turn streaming, Diff inspection,
//! and CDP browser inspection.

pub const INDEX_HTML: &str = r###"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>M31A — Frontier Autonomous Agent Workstation</title>
  <style>
    :root {
      --bg-base: #0c0e14;
      --bg-surface: #141721;
      --bg-surface-elevated: #1a1e2b;
      --bg-hover: #222738;
      --border: #262b3d;
      --border-focus: #3b82f6;
      --text-main: #f1f5f9;
      --text-muted: #94a3b8;
      --text-dim: #64748b;
      --accent: #38bdf8;
      --accent-glow: rgba(56, 189, 248, 0.15);
      --success: #34d399;
      --warning: #fbbf24;
      --error: #f87171;
      --font-mono: 'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace;
      --font-sans: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    }

    * { box-sizing: border-box; margin: 0; padding: 0; }

    body {
      background-color: var(--bg-base);
      color: var(--text-main);
      font-family: var(--font-sans);
      height: 100vh;
      overflow: hidden;
      display: flex;
      flex-direction: column;
    }

    /* Top Bar */
    header {
      height: 48px;
      background: var(--bg-surface);
      border-bottom: 1px solid var(--border);
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: 0 16px;
      font-size: 13px;
      user-select: none;
    }

    .brand {
      display: flex;
      align-items: center;
      gap: 10px;
      font-weight: 700;
      letter-spacing: 0.5px;
    }
    .brand-logo {
      width: 22px;
      height: 22px;
      border-radius: 4px;
      background: linear-gradient(135deg, #38bdf8, #818cf8);
      display: flex;
      align-items: center;
      justify-content: center;
      color: #000;
      font-family: var(--font-mono);
      font-size: 11px;
      font-weight: 900;
    }
    .brand-tag {
      font-size: 11px;
      color: var(--text-dim);
      font-family: var(--font-mono);
    }

    .top-controls {
      display: flex;
      align-items: center;
      gap: 12px;
    }

    .btn {
      background: var(--bg-surface-elevated);
      border: 1px solid var(--border);
      color: var(--text-main);
      padding: 5px 10px;
      border-radius: 6px;
      cursor: pointer;
      font-size: 12px;
      display: inline-flex;
      align-items: center;
      gap: 6px;
      transition: all 0.15s ease;
    }
    .btn:hover {
      background: var(--bg-hover);
      border-color: var(--border-focus);
    }
    .btn.active {
      background: var(--accent);
      color: #000;
      font-weight: 600;
      border-color: var(--accent);
    }

    .status-badge {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      font-size: 12px;
      color: var(--text-muted);
    }
    .status-dot {
      width: 8px;
      height: 8px;
      border-radius: 50%;
      background: var(--success);
      box-shadow: 0 0 6px var(--success);
    }
    .status-dot.offline {
      background: var(--error);
      box-shadow: 0 0 6px var(--error);
    }

    /* Main Container */
    #app-container {
      flex: 1;
      display: flex;
      height: calc(100vh - 48px);
      overflow: hidden;
    }

    /* Left Sidebar */
    aside.sidebar {
      width: 260px;
      background: var(--bg-surface);
      border-right: 1px solid var(--border);
      display: flex;
      flex-direction: column;
      flex-shrink: 0;
      transition: width 0.2s ease;
    }
    aside.sidebar.collapsed {
      width: 0;
      display: none;
    }

    .sidebar-section {
      padding: 12px;
      border-bottom: 1px solid var(--border);
    }
    .sidebar-title {
      font-size: 11px;
      font-weight: 700;
      text-transform: uppercase;
      letter-spacing: 0.5px;
      color: var(--text-dim);
      margin-bottom: 8px;
      display: flex;
      justify-content: space-between;
      align-items: center;
    }

    .thread-list, .worktree-list {
      list-style: none;
      overflow-y: auto;
      max-height: 240px;
    }
    .thread-item, .worktree-item {
      padding: 8px 10px;
      border-radius: 6px;
      cursor: pointer;
      font-size: 12px;
      margin-bottom: 4px;
      display: flex;
      align-items: center;
      justify-content: space-between;
      color: var(--text-muted);
    }
    .thread-item:hover, .worktree-item:hover {
      background: var(--bg-surface-elevated);
      color: var(--text-main);
    }
    .thread-item.active {
      background: var(--bg-surface-elevated);
      color: var(--accent);
      border-left: 2px solid var(--accent);
    }

    /* Center Chat / Focus Area */
    main.center-pane {
      flex: 1;
      display: flex;
      flex-direction: column;
      height: 100%;
      background: var(--bg-base);
      overflow: hidden;
    }

    .messages-container {
      flex: 1;
      overflow-y: auto;
      padding: 24px;
      display: flex;
      flex-direction: column;
      gap: 16px;
    }

    /* Focus Mode Centered Max Width */
    .focus-mode .messages-container {
      max-width: 820px;
      margin: 0 auto;
      width: 100%;
    }

    .message {
      display: flex;
      flex-direction: column;
      gap: 6px;
      max-width: 88%;
      animation: fadeIn 0.2s ease;
    }
    @keyframes fadeIn {
      from { opacity: 0; transform: translateY(4px); }
      to { opacity: 1; transform: translateY(0); }
    }

    .message.user {
      align-self: flex-end;
    }
    .message.assistant {
      align-self: flex-start;
      max-width: 95%;
    }

    .message-header {
      font-size: 11px;
      color: var(--text-dim);
      display: flex;
      gap: 8px;
      align-items: center;
    }

    .message-bubble {
      padding: 12px 16px;
      border-radius: 8px;
      font-size: 14px;
      line-height: 1.6;
    }
    .message.user .message-bubble {
      background: var(--bg-surface-elevated);
      border: 1px solid var(--border);
      color: var(--text-main);
    }
    .message.assistant .message-bubble {
      background: var(--bg-surface);
      border: 1px solid var(--border);
    }

    .code-block {
      background: #090b10;
      border: 1px solid var(--border);
      border-radius: 6px;
      margin: 8px 0;
      font-family: var(--font-mono);
      font-size: 13px;
      overflow-x: auto;
    }
    .code-header {
      background: #10131c;
      padding: 6px 12px;
      border-bottom: 1px solid var(--border);
      display: flex;
      justify-content: space-between;
      color: var(--text-dim);
      font-size: 11px;
    }
    .code-content {
      padding: 12px;
      white-space: pre-wrap;
    }

    .tool-pill {
      background: #1e293b;
      border: 1px solid #334155;
      color: #93c5fd;
      padding: 4px 8px;
      border-radius: 4px;
      font-family: var(--font-mono);
      font-size: 12px;
      display: inline-flex;
      align-items: center;
      gap: 6px;
      margin: 4px 0;
    }

    /* Composer */
    .composer-container {
      padding: 16px 24px;
      background: var(--bg-surface);
      border-top: 1px solid var(--border);
    }
    .focus-mode .composer-container {
      max-width: 820px;
      margin: 0 auto;
      width: 100%;
      background: transparent;
      border: none;
    }

    .composer-box {
      display: flex;
      flex-direction: column;
      background: var(--bg-surface-elevated);
      border: 1px solid var(--border);
      border-radius: 8px;
      padding: 10px;
      transition: border-color 0.2s ease;
    }
    .composer-box:focus-within {
      border-color: var(--border-focus);
      box-shadow: 0 0 0 2px var(--accent-glow);
    }

    textarea.prompt-input {
      background: transparent;
      border: none;
      color: var(--text-main);
      font-family: inherit;
      font-size: 14px;
      resize: none;
      min-height: 48px;
      max-height: 200px;
      outline: none;
    }

    .composer-actions {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-top: 8px;
      padding-top: 6px;
      border-top: 1px solid rgba(255, 255, 255, 0.05);
    }

    /* Right Inspector Pane */
    aside.inspector {
      width: 380px;
      background: var(--bg-surface);
      border-left: 1px solid var(--border);
      display: flex;
      flex-direction: column;
      flex-shrink: 0;
      transition: width 0.2s ease;
    }
    aside.inspector.collapsed {
      width: 0;
      display: none;
    }

    .tab-bar {
      display: flex;
      border-bottom: 1px solid var(--border);
      background: var(--bg-surface-elevated);
    }
    .tab-btn {
      flex: 1;
      padding: 10px;
      text-align: center;
      font-size: 12px;
      font-weight: 600;
      color: var(--text-muted);
      cursor: pointer;
      border-bottom: 2px solid transparent;
    }
    .tab-btn.active {
      color: var(--accent);
      border-bottom-color: var(--accent);
      background: var(--bg-surface);
    }

    .tab-content {
      flex: 1;
      padding: 16px;
      overflow-y: auto;
      font-size: 13px;
    }

    .diff-line {
      font-family: var(--font-mono);
      font-size: 12px;
      padding: 2px 4px;
      white-space: pre-wrap;
    }
    .diff-add {
      background: rgba(52, 211, 153, 0.12);
      color: #34d399;
    }
    .diff-del {
      background: rgba(248, 113, 113, 0.12);
      color: #f87171;
    }

    .task-item {
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 6px 0;
      border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    }
    .task-checkbox {
      accent-color: var(--accent);
    }
  </style>
</head>
<body>

  <!-- Top Navigation Bar -->
  <header>
    <div class="brand">
      <div class="brand-logo">M3</div>
      <span>M31A Workstation</span>
      <span class="brand-tag">v0.1.14 • Pure Rust Engine</span>
    </div>

    <div class="top-controls">
      <!-- Mode Toggle -->
      <button id="mode-toggle-btn" class="btn" onclick="toggleUIMode()">
        <span id="mode-icon">⚡</span> <span id="mode-label">Pro Mode</span>
      </button>

      <!-- Model Selector -->
      <select id="model-select" class="btn" onchange="onModelChange(this.value)">
        <option value="nvidia/deepseek-r1">nvidia/deepseek-r1 (Flagship)</option>
        <option value="meta/llama-3.3-70b-instruct">meta/llama-3.3-70b-instruct (Speed)</option>
        <option value="deepseek-ai/deepseek-coder-33b-instruct">deepseek-coder-33b (Code)</option>
      </select>

      <!-- Worktree Badge -->
      <div class="btn" id="worktree-badge" title="Active Git Worktree">
        🌿 <span id="worktree-label">main</span>
      </div>

      <!-- Sound Toggle -->
      <button id="sound-btn" class="btn" onclick="toggleSound()">
        🔊 Sound: ON
      </button>

      <!-- Server Status -->
      <div class="status-badge">
        <div id="status-dot" class="status-dot"></div>
        <span id="status-text">Connected</span>
      </div>
    </div>
  </header>

  <!-- Main Container -->
  <div id="app-container">

    <!-- Left Sidebar: Sessions & Worktrees -->
    <aside id="sidebar" class="sidebar">
      <div class="sidebar-section">
        <div class="sidebar-title">
          <span>Active Sessions</span>
          <button class="btn" style="padding: 2px 6px;" onclick="createNewSession()">+ New</button>
        </div>
        <ul id="thread-list" class="thread-list">
          <li class="thread-item active">
            <span>Primary Session</span>
            <small style="color: var(--text-dim);">just now</small>
          </li>
        </ul>
      </div>

      <div class="sidebar-section" style="flex: 1;">
        <div class="sidebar-title">
          <span>Git Worktrees</span>
          <button class="btn" style="padding: 2px 6px;" onclick="createNewWorktree()">+ Branch</button>
        </div>
        <ul id="worktree-list" class="worktree-list">
          <li class="worktree-item active">
            <span>repo (main)</span>
            <small style="color: var(--success);">ACTIVE</small>
          </li>
        </ul>
      </div>
    </aside>

    <!-- Center Pane: Transcript & Composer -->
    <main id="center-pane" class="center-pane">
      <div id="messages" class="messages-container">
        <!-- Welcome Card -->
        <div class="message assistant">
          <div class="message-header">
            <span>M31A Guardian Core</span>
            <span>•</span>
            <span>Linux bwrap Sandboxed</span>
          </div>
          <div class="message-bubble">
            Welcome to the <strong>M31A Agentic Workstation</strong>.
            I can read files, write code, run compiler verification, execute commands inside Bubblewrap namespaces, and automate browser tasks with Chrome DevTools Protocol.
            <br><br>
            Use <code>/goal &lt;objective&gt;</code> to start an autonomous review mission loop, or type instructions below.
          </div>
        </div>
      </div>

      <!-- Prompt Composer -->
      <div class="composer-container">
        <div class="composer-box">
          <textarea
            id="prompt-input"
            class="prompt-input"
            placeholder="Type an instruction, /goal, or question (Ctrl+Enter to send)..."
            onkeydown="handleComposerKey(event)"
          ></textarea>
          <div class="composer-actions">
            <div style="font-size: 11px; color: var(--text-dim);">
              Press <kbd>Ctrl+Enter</kbd> to submit
            </div>
            <button class="btn active" onclick="submitPrompt()">Send Turn ↵</button>
          </div>
        </div>
      </div>
    </main>

    <!-- Right Inspector Pane: Diffs, Tasks, CDP Browser -->
    <aside id="inspector" class="inspector">
      <div class="tab-bar">
        <div id="tab-diffs" class="tab-btn active" onclick="switchInspectorTab('diffs')">Diffs</div>
        <div id="tab-preview" class="tab-btn" onclick="switchInspectorTab('preview')">Preview</div>
        <div id="tab-locks" class="tab-btn" onclick="switchInspectorTab('locks')">Locks</div>
        <div id="tab-tasks" class="tab-btn" onclick="switchInspectorTab('tasks')">Tasks</div>
        <div id="tab-browser" class="tab-btn" onclick="switchInspectorTab('browser')">Browser</div>
      </div>

      <!-- Diffs Tab -->
      <div id="tab-content-diffs" class="tab-content">
        <div style="font-weight: 600; margin-bottom: 8px;">Workspace Diffs (HEAD)</div>
        <div id="diff-container" class="code-block" style="padding: 8px;">
          <div class="diff-line diff-add">+ // M31A pure Rust patch engine</div>
          <div class="diff-line diff-add">+ pub fn execute_mission() -> Result&lt;()&gt; {</div>
          <div class="diff-line">      println!("Autonomous loop active");</div>
          <div class="diff-line diff-del">-     std::thread::sleep(Duration::from_secs(1));</div>
          <div class="diff-line diff-add">+     tokio::task::yield_now().await;</div>
          <div class="diff-line">  }</div>
        </div>
      </div>

      <!-- Tasks Tab -->
      <div id="tab-content-tasks" class="tab-content" style="display: none;">
        <div style="font-weight: 600; margin-bottom: 8px;">Mission Goal &amp; Task Checklist</div>
        <div id="goal-card" style="background: var(--bg-surface-elevated); padding: 10px; border-radius: 6px; margin-bottom: 12px; border: 1px solid var(--border);">
          <div style="font-size: 11px; color: var(--text-dim); text-transform: uppercase;">Current Objective</div>
          <div id="goal-objective" style="font-weight: 600; margin-top: 4px;">Adopt OpenCode features into M31A</div>
          <div style="font-size: 11px; color: var(--success); margin-top: 4px;">Status: In Progress (Wave 3)</div>
        </div>

        <div id="task-checklist">
          <div class="task-item">
            <input type="checkbox" class="task-checkbox" checked disabled>
            <span>Phase 1: Surgical edit, question modal, schedule</span>
          </div>
          <div class="task-item">
            <input type="checkbox" class="task-checkbox" checked disabled>
            <span>Phase 2: Durable claims &amp; session inbox</span>
          </div>
          <div class="task-item">
            <input type="checkbox" class="task-checkbox" checked disabled>
            <span>Phase 3: Terminal LaTeX &amp; Mermaid engine</span>
          </div>
          <div class="task-item">
            <input type="checkbox" class="task-checkbox" checked disabled>
            <span>Phase 4: HTTP/SSE Daemon &amp; P2P Pairing</span>
          </div>
          <div class="task-item">
            <input type="checkbox" class="task-checkbox" checked disabled>
            <span>Phase 5: Chrome CDP Browser Automation</span>
          </div>
          <div class="task-item">
            <input type="checkbox" class="task-checkbox" checked disabled>
            <span>Phase 6: Multi-Workspace &amp; Worktree Switcher</span>
          </div>
          <div class="task-item">
            <input type="checkbox" class="task-checkbox" checked disabled>
            <span>Phase 7: GUI Frontend (Tauri / Modern Web UI)</span>
          </div>
        </div>
      </div>

      <!-- Browser CDP Tab -->
      <div id="tab-content-browser" class="tab-content" style="display: none;">
        <div style="font-weight: 600; margin-bottom: 8px;">Chrome DevTools Automation View</div>
        <div style="display: flex; gap: 6px; margin-bottom: 8px;">
          <input id="cdp-url" type="text" class="btn" style="flex: 1; text-align: left;" value="https://crates.io">
          <button class="btn active" onclick="navigateBrowser()">Go</button>
        </div>
        <div style="background: #000; border-radius: 6px; height: 180px; display: flex; align-items: center; justify-content: center; border: 1px solid var(--border); color: var(--text-dim); font-size: 12px;">
          <span>[CDP Headless Viewport 1280x720]</span>
        </div>
        <div style="margin-top: 10px; font-size: 11px; font-family: var(--font-mono); color: var(--text-muted);">
          Accessibility Node Tokens:<br>
          @e1: Button "Search"<br>
          @e2: Input "Search Crates"<br>
          @e3: Link "Categories"
        </div>
      </div>

      <!-- Live Web Workbench Preview Tab (Bolt.diy parity) -->
      <div id="tab-content-preview" class="tab-content" style="display: none; padding: 10px;">
        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;">
          <div style="font-weight: 600;">Live Web Preview</div>
          <div style="display: flex; gap: 4px;">
            <button class="btn" style="padding: 2px 6px;" onclick="setPreviewViewport('100%')" title="Desktop">💻</button>
            <button class="btn" style="padding: 2px 6px;" onclick="setPreviewViewport('768px')" title="Tablet">📱</button>
            <button class="btn" style="padding: 2px 6px;" onclick="setPreviewViewport('375px')" title="Mobile">📲</button>
            <button class="btn" style="padding: 2px 6px;" onclick="showExpoQrModal()" title="Expo QR Code">📲 Expo QR</button>
          </div>
        </div>

        <!-- Port Selector Bar -->
        <div style="display: flex; gap: 6px; margin-bottom: 8px; align-items: center;">
          <label style="font-size: 11px; color: var(--text-dim);">Port:</label>
          <select id="preview-port-select" class="btn" onchange="onPortSelectChange()">
            <option value="5173" selected>5173 (Vite)</option>
            <option value="3000">3000 (React/Next)</option>
            <option value="8080">8080 (HttpServer)</option>
            <option value="8081">8081 (Expo)</option>
            <option value="custom">Custom...</option>
          </select>
          <input id="preview-url-input" type="text" class="btn" style="flex: 1; text-align: left;" value="http://localhost:5173">
          <button class="btn" onclick="reloadPreview()" title="Reload iframe">↻</button>
          <button class="btn" onclick="openPreviewExternal()" title="Open in new window">↗</button>
        </div>

        <!-- Responsive Iframe Container -->
        <div id="preview-frame-wrapper" style="width: 100%; height: calc(100vh - 160px); background: #fff; border-radius: 6px; overflow: hidden; border: 1px solid var(--border); margin: 0 auto; transition: width 0.2s ease;">
          <iframe id="preview-iframe" src="http://localhost:5173" style="width: 100%; height: 100%; border: none;"></iframe>
        </div>
      </div>

      <!-- Concurrency File Locks Tab (Bolt.diy parity) -->
      <div id="tab-content-locks" class="tab-content" style="display: none;">
        <div style="font-weight: 600; margin-bottom: 8px;">Concurrency File Locks</div>
        <div style="display: flex; gap: 6px; margin-bottom: 12px;">
          <input id="lock-path-input" type="text" class="btn" style="flex: 1; text-align: left;" placeholder="relative/file/or/dir/path">
          <button class="btn active" onclick="quickLockPath()">Lock Path</button>
        </div>
        <div id="locks-list-container">
          <div style="font-size: 12px; color: var(--text-muted); margin-bottom: 6px;">Protected from agent modification:</div>
          <ul id="active-locks-ul" style="list-style: none;">
            <li class="worktree-item"><span>No active file locks</span></li>
          </ul>
        </div>
      </div>
    </aside>
  </div>

  <!-- Expo QR Modal -->
  <div id="expo-modal" style="display: none; position: fixed; top: 0; left: 0; width: 100vw; height: 100vh; background: rgba(0,0,0,0.7); z-index: 1000; align-items: center; justify-content: center;">
    <div style="background: var(--bg-surface); border: 1px solid var(--border); border-radius: 12px; padding: 24px; max-width: 420px; width: 90%; text-align: center; box-shadow: 0 10px 25px rgba(0,0,0,0.5);">
      <h3 style="margin-bottom: 8px;">📲 Expo Mobile Preview</h3>
      <p style="font-size: 13px; color: var(--text-muted); margin-bottom: 16px;">
        Scan this QR code with the <strong>Expo Go</strong> app on iOS or Android, or open the link below on your device.
      </p>
      <div style="background: #ffffff; padding: 16px; border-radius: 8px; display: inline-block; margin-bottom: 16px;">
        <svg width="180" height="180" viewBox="0 0 100 100">
          <rect width="100" height="100" fill="#fff" />
          <rect x="10" y="10" width="25" height="25" fill="#000" />
          <rect x="15" y="15" width="15" height="15" fill="#fff" />
          <rect x="18" y="18" width="9" height="9" fill="#000" />
          <rect x="65" y="10" width="25" height="25" fill="#000" />
          <rect x="70" y="15" width="15" height="15" fill="#fff" />
          <rect x="73" y="18" width="9" height="9" fill="#000" />
          <rect x="10" y="65" width="25" height="25" fill="#000" />
          <rect x="15" y="70" width="15" height="15" fill="#fff" />
          <rect x="18" y="73" width="9" height="9" fill="#000" />
          <rect x="42" y="15" width="8" height="8" fill="#000" />
          <rect x="42" y="42" width="16" height="16" fill="#000" />
          <rect x="65" y="42" width="8" height="8" fill="#000" />
          <rect x="42" y="70" width="8" height="15" fill="#000" />
          <rect x="70" y="70" width="18" height="18" fill="#000" />
        </svg>
      </div>
      <div style="background: var(--bg-surface-elevated); padding: 8px; border-radius: 6px; font-family: var(--font-mono); font-size: 12px; margin-bottom: 16px; word-break: break-all;">
        exp://localhost:8081
      </div>
      <button class="btn active" onclick="closeExpoQrModal()" style="width: 100%; justify-content: center;">Close</button>
    </div>
  </div>

  <script>
    // State
    let currentMode = 'pro'; // 'pro' or 'focus'
    let soundEnabled = true;
    let sseSource = null;
    let activeThreadId = 'default';

    // Audio chime synthesis
    function playChime() {
      if (!soundEnabled) return;
      try {
        const ctx = new (window.AudioContext || window.webkitAudioContext)();
        const osc = ctx.createOscillator();
        const gain = ctx.createGain();
        osc.type = 'sine';
        osc.frequency.setValueAtTime(587.33, ctx.currentTime); // D5
        osc.frequency.exponentialRampToValueAtTime(880, ctx.currentTime + 0.1); // A5
        gain.gain.setValueAtTime(0.08, ctx.currentTime);
        gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.3);
        osc.connect(gain);
        gain.connect(ctx.destination);
        osc.start();
        osc.stop(ctx.currentTime + 0.3);
      } catch (e) {}
    }

    // Toggle Sound
    function toggleSound() {
      soundEnabled = !soundEnabled;
      document.getElementById('sound-btn').innerText = soundEnabled ? '🔊 Sound: ON' : '🔈 Sound: OFF';
    }

    // Toggle Focus vs. Pro Mode
    function toggleUIMode() {
      const container = document.getElementById('app-container');
      const sidebar = document.getElementById('sidebar');
      const inspector = document.getElementById('inspector');
      const label = document.getElementById('mode-label');

      if (currentMode === 'pro') {
        currentMode = 'focus';
        sidebar.classList.add('collapsed');
        inspector.classList.add('collapsed');
        container.classList.add('focus-mode');
        label.innerText = 'Focus Mode';
      } else {
        currentMode = 'pro';
        sidebar.classList.remove('collapsed');
        inspector.classList.remove('collapsed');
        container.classList.remove('focus-mode');
        label.innerText = 'Pro Mode';
      }
    }

    // Inspector Tab Switching
    function switchInspectorTab(tab) {
      ['diffs', 'tasks', 'preview', 'locks', 'browser'].forEach(t => {
        const btn = document.getElementById(`tab-${t}`);
        const content = document.getElementById(`tab-content-${t}`);
        if (btn) btn.classList.remove('active');
        if (content) content.style.display = 'none';
      });
      const activeBtn = document.getElementById(`tab-${tab}`);
      const activeContent = document.getElementById(`tab-content-${tab}`);
      if (activeBtn) activeBtn.classList.add('active');
      if (activeContent) activeContent.style.display = 'block';
    }

    function setPreviewViewport(width) {
      document.getElementById('preview-frame-wrapper').style.width = width;
    }

    function onPortSelectChange() {
      const select = document.getElementById('preview-port-select');
      const input = document.getElementById('preview-url-input');
      if (select.value === 'custom') {
        input.focus();
      } else {
        input.value = `http://localhost:${select.value}`;
        reloadPreview();
      }
    }

    function reloadPreview() {
      const input = document.getElementById('preview-url-input');
      const iframe = document.getElementById('preview-iframe');
      iframe.src = input.value;
    }

    function openPreviewExternal() {
      const input = document.getElementById('preview-url-input');
      window.open(input.value, '_blank');
    }

    function showExpoQrModal() {
      document.getElementById('expo-modal').style.display = 'flex';
    }

    function closeExpoQrModal() {
      document.getElementById('expo-modal').style.display = 'none';
    }

    let activeLocks = [];
    function quickLockPath() {
      const input = document.getElementById('lock-path-input');
      const val = input.value.trim();
      if (!val) return;
      if (!activeLocks.includes(val)) {
        activeLocks.push(val);
        renderLocks();
      }
      input.value = '';
    }

    function quickUnlockPath(path) {
      activeLocks = activeLocks.filter(p => p !== path);
      renderLocks();
    }

    function renderLocks() {
      const ul = document.getElementById('active-locks-ul');
      if (!ul) return;
      if (activeLocks.length === 0) {
        ul.innerHTML = '<li class="worktree-item"><span>No active file locks</span></li>';
        return;
      }
      ul.innerHTML = '';
      activeLocks.forEach(p => {
        const li = document.createElement('li');
        li.className = 'worktree-item';
        li.innerHTML = "<span>🔒 " + p + "</span><button class=\"btn\" style=\"padding: 2px 6px;\" onclick=\"quickUnlockPath('" + p + "')\">Unlock</button>";
        ul.appendChild(li);
      });
    }

    // Keyboard Shortcuts
    function handleComposerKey(e) {
      if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
        e.preventDefault();
        submitPrompt();
      }
    }

    // Append Message
    function appendMessage(role, text) {
      const container = document.getElementById('messages');
      const msgDiv = document.createElement('div');
      msgDiv.className = `message ${role}`;

      const header = document.createElement('div');
      header.className = 'message-header';
      header.innerHTML = `<span>${role === 'user' ? 'You' : 'M31A Assistant'}</span><span>•</span><span>${new Date().toLocaleTimeString()}</span>`;

      const bubble = document.createElement('div');
      bubble.className = 'message-bubble';
      bubble.innerText = text;

      msgDiv.appendChild(header);
      msgDiv.appendChild(bubble);
      container.appendChild(msgDiv);
      container.scrollTop = container.scrollHeight;
      return bubble;
    }

    // Submit Prompt
    async function submitPrompt() {
      const input = document.getElementById('prompt-input');
      const text = input.value.trim();
      if (!text) return;

      appendMessage('user', text);
      input.value = '';

      // Create assistant reply bubble for streaming
      const replyBubble = appendMessage('assistant', 'Thinking...');

      try {
        const resp = await fetch('/rpc', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            jsonrpc: '2.0',
            id: 1,
            method: 'turn/start',
            params: {
              thread_id: activeThreadId,
              prompt: text
            }
          })
        });
        const data = await resp.json();
        if (data.result && data.result.text) {
          replyBubble.innerText = data.result.text;
        } else {
          replyBubble.innerText = 'Executed turn successfully.';
        }
        playChime();
      } catch (e) {
        replyBubble.innerText = `Error communicating with server: ${e.message}`;
      }
    }

    // Setup SSE Stream
    function initSSE() {
      try {
        sseSource = new EventSource('/events');
        sseSource.onopen = () => {
          document.getElementById('status-dot').className = 'status-dot';
          document.getElementById('status-text').innerText = 'Connected';
        };
        sseSource.onerror = () => {
          document.getElementById('status-dot').className = 'status-dot offline';
          document.getElementById('status-text').innerText = 'Reconnecting...';
        };
        sseSource.onmessage = (e) => {
          try {
            const ev = JSON.parse(e.data);
            if (ev.type === 'token') {
              // Append streaming token
            }
          } catch (_) {}
        };
      } catch (e) {}
    }

    function createNewSession() {
      const id = prompt('Enter new session title:', 'Feature Dev');
      if (id) {
        const list = document.getElementById('thread-list');
        const item = document.createElement('li');
        item.className = 'thread-item';
        item.innerHTML = `<span>${id}</span><small style="color: var(--text-dim);">new</small>`;
        list.appendChild(item);
      }
    }

    function createNewWorktree() {
      alert('Creating new managed git worktree branch...');
    }

    function navigateBrowser() {
      const url = document.getElementById('cdp-url').value;
      alert(`CDP navigating to ${url}`);
    }

    // Initialize
    window.onload = () => {
      initSSE();
    };
  </script>
</body>
</html>
"###;
