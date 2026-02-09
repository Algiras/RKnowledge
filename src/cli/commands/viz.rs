use anyhow::{Context, Result};
use console::{Emoji, style};
use indicatif::{ProgressBar, ProgressStyle};
use std::io::Write;
use std::time::Duration;

use crate::config::Config;
use crate::graph::neo4j::Neo4jClient;

static GRAPH: Emoji<'_, '_> = Emoji("🔗 ", "");
static BROWSER: Emoji<'_, '_> = Emoji("🌐 ", "");
static CHECK: Emoji<'_, '_> = Emoji("✅ ", "[OK] ");
static SPARKLE: Emoji<'_, '_> = Emoji("✨ ", "");

pub async fn run(_port: u16, tenant: Option<&str>) -> Result<()> {
    println!();
    println!(
        "{}",
        style(" RKnowledge - Graph Visualization ").bold().reverse()
    );
    println!();

    // Load configuration
    let config =
        Config::load().context("Failed to load configuration. Run 'rknowledge init' first.")?;

    // Connect to Neo4j and fetch data
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template(&format!("{}{{spinner:.green}} {{msg}}", GRAPH))
            .unwrap(),
    );
    spinner.enable_steady_tick(Duration::from_millis(100));
    spinner.set_message("Fetching graph from Neo4j...");

    let neo4j_client = Neo4jClient::new(&config.neo4j).await?;
    let (nodes, edges) = neo4j_client.fetch_graph(tenant).await?;

    spinner.finish_and_clear();
    println!(
        "{}Loaded {} nodes, {} edges",
        CHECK,
        style(nodes.len()).green().bold(),
        style(edges.len()).green().bold()
    );

    // Generate HTML visualization
    let html = generate_viz_html(&nodes, &edges)?;

    // Write to temp file and open in browser
    let temp_dir = std::env::temp_dir();
    let html_path = temp_dir.join("rknowledge_viz.html");

    let mut file = std::fs::File::create(&html_path)?;
    file.write_all(html.as_bytes())?;

    // Try to open in default browser
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&html_path)
            .spawn()
            .ok();
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&html_path)
            .spawn()
            .ok();
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", html_path.to_str().unwrap_or("")])
            .spawn()
            .ok();
    }

    println!();
    println!("{}Visualization opened in browser", BROWSER);
    println!();
    println!(
        "{}File: {}",
        SPARKLE,
        style(html_path.display()).cyan().underlined()
    );

    Ok(())
}

fn generate_viz_html(
    nodes: &[crate::graph::neo4j::GraphNode],
    edges: &[crate::graph::neo4j::GraphEdge],
) -> Result<String> {
    // Build nodes JSON
    let nodes_json: Vec<serde_json::Value> = nodes
        .iter()
        .map(|n| {
            serde_json::json!({
                "id": n.id,
                "label": n.label,
                "community": n.community.unwrap_or(0),
                "degree": n.degree.unwrap_or(1),
                "entityType": n.entity_type.as_deref().unwrap_or("concept"),
            })
        })
        .collect();

    // Build edges JSON
    let edges_json: Vec<serde_json::Value> = edges
        .iter()
        .map(|e| {
            let is_proximity = e.relation == "contextual proximity";
            serde_json::json!({
                "from": e.source,
                "to": e.target,
                "label": if is_proximity { "" } else { &e.relation },
                "fullLabel": e.relation,
                "value": e.weight,
                "isProximity": is_proximity,
            })
        })
        .collect();

    let explicit_count = edges
        .iter()
        .filter(|e| e.relation != "contextual proximity")
        .count();
    let proximity_count = edges.len() - explicit_count;

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>RKnowledge Graph Explorer</title>
    <style>
        @import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap');
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{ font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #050510; color: #c8c8e0; overflow: hidden; height: 100vh; display: flex; flex-direction: column; }}
        
        /* Layout */
        #header {{ padding: 12px 24px; background: rgba(18, 18, 42, 0.9); backdrop-filter: blur(16px); border-bottom: 1px solid rgba(255,255,255,0.08); display: flex; align-items: center; gap: 24px; z-index: 100; flex-shrink: 0; }}
        #header h1 {{ font-size: 1.1em; font-weight: 800; letter-spacing: -0.5px; background: linear-gradient(135deg, #ff6b8a, #a855f7); -webkit-background-clip: text; -webkit-text-fill-color: transparent; white-space: nowrap; cursor: pointer; }}
        
        .toolbar {{ display: flex; gap: 12px; align-items: center; flex: 1; }}
        #search-container {{ position: relative; width: 260px; }}
        #search {{ width: 100%; background: rgba(10, 10, 30, 0.6); border: 1px solid rgba(255,255,255,0.1); border-radius: 10px; color: #f0f0ff; padding: 9px 14px 9px 36px; font-size: 0.85em; transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1); outline: none; }}
        #search:focus {{ border-color: #a855f7; box-shadow: 0 0 15px rgba(168, 85, 247, 0.25); width: 320px; }}
        #search-icon {{ position: absolute; left: 12px; top: 50%; transform: translateY(-50%); color: #555577; pointer-events: none; }}

        .btn {{ background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.08); border-radius: 8px; color: #8888aa; padding: 8px 14px; font-size: 0.8em; font-weight: 500; cursor: pointer; transition: all 0.2s; white-space: nowrap; display: flex; align-items: center; gap: 6px; }}
        .btn:hover {{ background: rgba(255,255,255,0.06); border-color: rgba(255,255,255,0.2); color: #c8c8e0; }}
        .btn.active {{ background: rgba(168, 85, 247, 0.15); border-color: #a855f7; color: #a855f7; }}
        
        #main {{ flex: 1; position: relative; overflow: hidden; }}
        #graph {{ width: 100%; height: 100%; }}
        
        /* Stats & Feedback */
        #stats-bar {{ display: flex; align-items: center; gap: 16px; font-size: 0.75em; color: #555577; margin-left: auto; }}
        .stat-item {{ display: flex; align-items: center; gap: 6px; background: rgba(255,255,255,0.02); padding: 4px 10px; border-radius: 100px; border: 1px solid rgba(255,255,255,0.04); }}
        .stat-val {{ color: #a855f7; font-weight: 700; }}

        /* Loading / Error / Empty States */
        .overlay-center {{ position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); text-align: center; pointer-events: none; z-index: 50; }}
        #loading {{ color: #555577; }}
        .spinner {{ width: 48px; height: 48px; border: 3px solid rgba(168, 85, 247, 0.1); border-top-color: #a855f7; border-radius: 50%; animation: spin 1s linear infinite; margin: 0 auto 16px; }}
        @keyframes spin {{ to {{ transform: rotate(360deg); }} }}
        
        #empty-state {{ display: none; }}
        #empty-state h2 {{ color: #ff6b8a; font-size: 1.2em; margin-bottom: 8px; }}
        #empty-state p {{ color: #555577; font-size: 0.9em; max-width: 300px; }}

        /* Filtering Legend */
        #filter-sidebar {{ position: absolute; left: 16px; top: 16px; bottom: 16px; width: 220px; background: rgba(10, 10, 26, 0.85); backdrop-filter: blur(20px); border: 1px solid rgba(255,255,255,0.06); border-radius: 16px; display: flex; flex-direction: column; z-index: 20; box-shadow: 0 8px 32px rgba(0,0,0,0.4); }}
        .sidebar-header {{ padding: 18px 20px 12px; border-bottom: 1px solid rgba(255,255,255,0.04); font-size: 0.75em; font-weight: 700; text-transform: uppercase; letter-spacing: 1px; color: #a855f7; }}
        .filter-list {{ flex: 1; overflow-y: auto; padding: 10px; }}
        .filter-item {{ display: flex; align-items: center; gap: 10px; padding: 10px 12px; border-radius: 10px; cursor: pointer; transition: all 0.2s; margin-bottom: 4px; user-select: none; }}
        .filter-item:hover {{ background: rgba(255,255,255,0.04); }}
        .filter-dot {{ width: 10px; height: 10px; border-radius: 50%; box-shadow: 0 0 10px currentColor; }}
        .filter-label {{ font-size: 0.82em; color: #b0b0c8; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }}
        .filter-count {{ font-size: 0.75em; color: #444466; }}
        .filter-item.hidden {{ opacity: 0.4; text-decoration: line-through; }}

        /* Node Detail Card */
        #detail-card {{ position: absolute; top: 0; right: 0; width: 400px; height: 100%; background: rgba(8, 8, 20, 0.95); backdrop-filter: blur(24px); border-left: 1px solid rgba(255,255,255,0.08); z-index: 80; transform: translateX(100%); transition: transform 0.4s cubic-bezier(0.19, 1, 0.22, 1); display: flex; flex-direction: column; }}
        #detail-card.open {{ transform: translateX(0); }}
        .card-header {{ padding: 32px 24px 24px; border-bottom: 1px solid rgba(255,255,255,0.05); position: relative; }}
        .card-close {{ position: absolute; top: 16px; right: 16px; cursor: pointer; color: #444; font-size: 24px; }}
        .card-close:hover {{ color: #ff6b8a; }}
        .type-badge {{ font-size: 0.65em; font-weight: 700; text-transform: uppercase; letter-spacing: 1.5px; margin-bottom: 8px; display: flex; align-items: center; gap: 8px; }}
        .type-dot {{ width: 8px; height: 8px; border-radius: 50%; }}
        .card-title {{ font-size: 1.5em; font-weight: 800; color: #ffffff; letter-spacing: -0.5px; line-height: 1.2; }}
        
        .card-content {{ flex: 1; overflow-y: auto; padding: 24px; }}
        .section-title {{ font-size: 0.7em; font-weight: 700; color: #444466; text-transform: uppercase; letter-spacing: 1.5px; margin-bottom: 16px; border-bottom: 1px solid rgba(255,255,255,0.03); padding-bottom: 8px; }}
        .rel-item {{ display: flex; gap: 14px; margin-bottom: 18px; cursor: pointer; padding: 12px; border-radius: 12px; transition: all 0.2s; background: rgba(255,255,255,0.02); }}
        .rel-item:hover {{ background: rgba(168, 85, 247, 0.08); transform: translateY(-2px); }}
        .rel-info {{ flex: 1; }}
        .rel-target {{ font-weight: 600; font-size: 0.95em; color: #d0d0f0; }}
        .rel-type {{ font-size: 0.8em; color: #666688; margin-top: 4px; font-style: italic; }}
        .rel-meta {{ font-size: 0.7em; color: #444466; margin-top: 4px; }}

        /* Tooltip */
        #tooltip {{ display: none; position: fixed; background: rgba(10, 10, 20, 0.95); border: 1px solid rgba(168, 85, 247, 0.3); border-radius: 10px; padding: 10px 14px; z-index: 1000; pointer-events: none; backdrop-filter: blur(8px); box-shadow: 0 10px 25px rgba(0,0,0,0.5); }}
    </style>
</head>
<body>
    <div id="header">
        <h1 onclick="window.location.reload()">RKnowledge</h1>
        <div class="toolbar">
            <div id="search-container">
                <span id="search-icon">🔍</span>
                <input id="search" type="text" placeholder="Filter concepts..." />
            </div>
            <button class="btn active" id="toggleProximity">Proximity</button>
            <button class="btn" id="togglePhysics">Freeze</button>
            <div id="stats-bar">
                <div class="stat-item"><span class="stat-val">{}</span> nodes</div>
                <div class="stat-item"><span class="stat-val">{}</span> explicit</div>
                <div class="stat-item"><span class="stat-val">{}</span> proximity</div>
            </div>
        </div>
    </div>
    
    <div id="main">
        <div id="graph"></div>
        
        <div id="loading" class="overlay-center">
            <div class="spinner"></div>
            <div>Constructing neural network...</div>
        </div>
        
        <div id="empty-state" class="overlay-center">
            <h2>Graph is Empty</h2>
            <p>No concepts extracted for this tenant. Try running 'rknowledge build' or 'rknowledge add'.</p>
        </div>
        
        <div id="filter-sidebar">
            <div class="sidebar-header">Entity Types</div>
            <div class="filter-list" id="type-filters"></div>
        </div>

        <div id="detail-card">
            <div class="card-header">
                <div class="card-close" onclick="closeDetail()">&times;</div>
                <div class="type-badge"><span class="type-dot"></span><span class="type-text"></span></div>
                <div class="card-title"></div>
            </div>
            <div class="card-content" id="card-relations"></div>
        </div>
    </div>
    
    <div id="tooltip"></div>

    <script>
        // Load Vis.js
        const CDNS = [
            'https://cdnjs.cloudflare.com/ajax/libs/vis-network/9.1.9/vis-network.min.js',
            'https://unpkg.com/vis-network@9.1.9/standalone/umd/vis-network.min.js'
        ];
        function loadScript(urls, idx) {{
            if (idx >= urls.length) {{ 
                document.getElementById('loading').textContent = 'Loading failed. Check connection.';
                return;
            }}
            const s = document.createElement('script');
            s.src = urls[idx];
            s.onload = initApp;
            s.onerror = () => loadScript(urls, idx + 1);
            document.head.appendChild(s);
        }}
        loadScript(CDNS, 0);

        // Data injected by Rust
        const rawNodes = {};
        const rawEdges = {};

        function initApp() {{
            const loading = document.getElementById('loading');
            const emptyState = document.getElementById('empty-state');
            
            if (rawNodes.length === 0) {{
                loading.style.display = 'none';
                emptyState.style.display = 'block';
                return;
            }}

            // Preprocessing
            const colors = [
                '#6366f1', '#ec4899', '#14b8a6', '#f59e0b', '#8b5cf6', '#06b6d4', '#ef4444', '#22c55e', '#f97316'
            ];
            const typeMap = {{}};
            let colorIdx = 0;
            const nodesByType = {{}};

            rawNodes.forEach(n => {{
                if (!typeMap[n.entityType]) {{
                    typeMap[n.entityType] = colors[colorIdx % colors.length];
                    colorIdx++;
                }}
                n.originalColor = typeMap[n.entityType];
                n.color = {{ background: n.originalColor, border: n.originalColor, highlight: n.originalColor, hover: n.originalColor }};
                n.font = {{ color: '#c8c8e0', size: 14, strokeWidth: 0 }};
                n.shadow = {{ enabled: true, color: 'rgba(0,0,0,0.5)', size: 8 }};
                
                if (!nodesByType[n.entityType]) nodesByType[n.entityType] = [];
                nodesByType[n.entityType].push(n.id);
            }});

            rawEdges.forEach(e => {{
                if (e.isProximity) {{
                    e.color = {{ color: 'rgba(60,60,100,0.15)', highlight: 'rgba(120,80,200,0.3)' }};
                    e.width = 0.5;
                }} else {{
                    e.color = {{ color: 'rgba(168,85,247,0.4)', highlight: '#a855f7' }};
                    e.width = Math.max(1.5, Math.min(5, e.value / 2));
                    e.arrows = {{ to: {{ enabled: true, scaleFactor: 0.5 }} }};
                }}
                e.smooth = {{ type: 'curvedCW', roundness: 0.1 }};
            }});

            const nodes = new vis.DataSet(rawNodes);
            const edges = new vis.DataSet(rawEdges);
            const container = document.getElementById('graph');
            
            const options = {{
                nodes: {{ shape: 'dot' }},
                edges: {{ hoverWidth: 1.5 }},
                physics: {{
                    forceAtlas2Based: {{ 
                        gravitationalConstant: -150, 
                        centralGravity: 0.005, 
                        springLength: 200, 
                        springConstant: 0.08 
                    }},
                    solver: 'forceAtlas2Based',
                    stabilization: {{ iterations: 300 }}
                }},
                interaction: {{ hover: true, tooltipDelay: 200, hideEdgesOnDrag: true }}
            }};

            const network = new vis.Network(container, {{ nodes, edges }}, options);
            loading.style.display = 'none';

            // ─── Filtering ───
            const typeContainer = document.getElementById('type-filters');
            const hiddenTypes = new Set();

            Object.keys(typeMap).sort().forEach(type => {{
                const item = document.createElement('div');
                item.className = 'filter-item';
                item.innerHTML = `
                    <span class="filter-dot" style="background: ${{typeMap[type]}}; color: ${{typeMap[type]}}"></span>
                    <span class="filter-label">${{type}}</span>
                    <span class="filter-count">${{nodesByType[type].length}}</span>
                `;
                item.onclick = () => {{
                    if (hiddenTypes.has(type)) hiddenTypes.delete(type);
                    else hiddenTypes.add(type);
                    item.classList.toggle('hidden');
                    updateVisibility();
                }};
                typeContainer.appendChild(item);
            }});

            function updateVisibility() {{
                const q = document.getElementById('search').value.toLowerCase();
                nodes.update(rawNodes.map(n => ({{
                    id: n.id,
                    hidden: hiddenTypes.has(n.entityType) || (q && !n.label.toLowerCase().includes(q))
                }})));
            }}

            document.getElementById('search').oninput = updateVisibility;

            // ─── Toggles ───
            let showProximity = true;
            document.getElementById('toggleProximity').onclick = (e) => {{
                showProximity = !showProximity;
                e.target.classList.toggle('active', showProximity);
                edges.update(rawEdges.filter(ed => ed.isProximity).map(ed => ({{ id: ed.id, hidden: !showProximity }})));
            }};

            let physicsOn = true;
            document.getElementById('togglePhysics').onclick = (e) => {{
                physicsOn = !physicsOn;
                e.target.classList.toggle('active', !physicsOn);
                e.target.textContent = physicsOn ? 'Freeze' : 'Unfreeze';
                network.setOptions({{ physics: {{ enabled: physicsOn }} }});
            }};

            // ─── Interaction ───
            network.on('click', (p) => {{
                if (p.nodes.length > 0) openDetail(p.nodes[0]);
                else closeDetail();
            }});

            const detailCard = document.getElementById('detail-card');
            function openDetail(nodeId) {{
                const n = nodes.get(nodeId);
                detailCard.querySelector('.type-dot').style.background = n.originalColor;
                detailCard.querySelector('.type-text').textContent = n.entityType;
                detailCard.querySelector('.type-badge').style.color = n.originalColor;
                detailCard.querySelector('.card-title').textContent = n.label;
                
                // Fetch connected
                const connEdges = network.getConnectedEdges(nodeId);
                let html = '<div class="section-title">Relations</div>';
                
                const neighbors = [];
                connEdges.forEach(eId => {{
                    const e = edges.get(eId);
                    const tid = e.from === nodeId ? e.to : e.from;
                    const target = nodes.get(tid);
                    neighbors.push({{ target, relation: e.fullLabel || e.label || 'related', proximity: e.isProximity }});
                }});

                neighbors.sort((a,b) => b.proximity - a.proximity); // Explicit first
                
                neighbors.forEach(nb => {{
                    html += `
                        <div class="rel-item" onclick="focusNode('${{nb.target.id}}')">
                            <div class="rel-info">
                                <div class="rel-target">${{nb.target.label}}</div>
                                <div class="rel-type">${{nb.relation}}</div>
                                <div class="rel-meta">${{nb.target.entityType}}</div>
                            </div>
                        </div>
                    `;
                }});
                
                document.getElementById('card-relations').innerHTML = html;
                detailCard.classList.add('open');
            }}

            window.closeDetail = () => detailCard.classList.remove('open');
            window.focusNode = (id) => {{
                network.focus(id, {{ scale: 1.2, animation: true }});
                network.selectNodes([id]);
                openDetail(id);
            }};

            // Tooltip on hover
            const tooltip = document.getElementById('tooltip');
            network.on('hoverNode', p => {{
                const n = nodes.get(p.node);
                tooltip.innerHTML = `<div style="color:white; font-weight:700">${{n.label}}</div><div style="font-size:0.75em; color:#888">${{n.entityType}}</div>`;
                tooltip.style.display = 'block';
            }});
            network.on('blurNode', () => tooltip.style.display = 'none');
            network.on('mousemove', e => {{
                tooltip.style.left = (e.event.pageX + 15) + 'px';
                tooltip.style.top = (e.event.pageY + 15) + 'px';
            }});
        }}
    </script>
</body>
</html>"#,
        nodes.len(),
        explicit_count,
        proximity_count,
        serde_json::to_string(&nodes_json)?,
        serde_json::to_string(&edges_json)?
    );

    Ok(html)
}
