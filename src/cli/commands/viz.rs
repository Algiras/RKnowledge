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

pub async fn run(_port: u16) -> Result<()> {
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
    let (nodes, edges) = neo4j_client.fetch_graph().await?;

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
                "group": n.community.unwrap_or(0),
                "value": n.degree.unwrap_or(1),
                "entityType": n.entity_type.as_deref().unwrap_or("concept"),
            })
        })
        .collect();

    // Build edges JSON -- truncate long labels, mark proximity edges
    let edges_json: Vec<serde_json::Value> = edges
        .iter()
        .map(|e| {
            let is_proximity = e.relation == "contextual proximity";
            let short_label = if is_proximity {
                String::new() // no label for proximity edges
            } else if e.relation.len() > 40 {
                format!("{}...", &e.relation[..37])
            } else {
                e.relation.clone()
            };
            serde_json::json!({
                "from": e.source,
                "to": e.target,
                "label": short_label,
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
    <title>RKnowledge Graph</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{ font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0a0a1a; color: #c8c8e0; overflow: hidden; }}
        #header {{ padding: 10px 20px; background: linear-gradient(135deg, #12122a 0%, #1a1a3e 100%); border-bottom: 1px solid #2a2a5a; display: flex; align-items: center; gap: 16px; z-index: 30; position: relative; }}
        #header h1 {{ font-size: 1.1em; font-weight: 600; color: #ff6b8a; white-space: nowrap; }}
        .toolbar {{ display: flex; gap: 8px; align-items: center; flex: 1; }}
        #search {{ background: #0e0e22; border: 1px solid #2a2a5a; border-radius: 6px; color: #c8c8e0; padding: 5px 10px; font-size: 0.8em; width: 180px; outline: none; transition: border-color 0.2s; }}
        #search:focus {{ border-color: #ff6b8a; }}
        .btn {{ background: #1a1a3e; border: 1px solid #2a2a5a; border-radius: 6px; color: #8888aa; padding: 5px 10px; font-size: 0.75em; cursor: pointer; transition: all 0.2s; white-space: nowrap; user-select: none; }}
        .btn:hover {{ border-color: #ff6b8a; color: #ff6b8a; }}
        .btn.active {{ background: #2a1a3e; border-color: #ff6b8a; color: #ff6b8a; }}
        .stats {{ font-size: 0.75em; color: #555577; white-space: nowrap; margin-left: auto; }}
        #graph {{ width: 100%; height: calc(100vh - 42px); }}

        /* Loading / Error */
        #loading {{ position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); text-align: center; color: #555577; }}
        #loading .spinner {{ width: 32px; height: 32px; border: 2px solid #1a1a3e; border-top-color: #ff6b8a; border-radius: 50%; animation: spin 0.7s linear infinite; margin: 0 auto 12px; }}
        @keyframes spin {{ to {{ transform: rotate(360deg); }} }}
        #error {{ display: none; position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); text-align: center; color: #ff6b8a; max-width: 420px; font-size: 0.9em; }}

        /* Hover tooltip (compact preview) */
        #tooltip {{ display: none; position: absolute; background: #14142eee; border: 1px solid #2a2a5a; border-radius: 8px; padding: 10px 12px; font-size: 0.8em; max-width: 260px; box-shadow: 0 8px 32px rgba(0,0,0,0.6); pointer-events: none; z-index: 20; backdrop-filter: blur(8px); }}
        #tooltip .tt-type {{ font-size: 0.8em; color: #666688; margin-bottom: 3px; display: flex; align-items: center; gap: 5px; }}
        #tooltip .tt-dot {{ width: 7px; height: 7px; border-radius: 50%; display: inline-block; }}
        #tooltip .tt-name {{ color: #e0e0f0; font-weight: 600; font-size: 1em; }}
        #tooltip .tt-hint {{ font-size: 0.75em; color: #444466; margin-top: 5px; }}

        /* Detail card (click panel) */
        #card {{ display: none; position: fixed; top: 42px; right: 0; width: 360px; height: calc(100vh - 42px); background: #0e0e22; border-left: 1px solid #2a2a5a; z-index: 15; flex-direction: column; overflow: hidden; }}
        #card.open {{ display: flex; }}
        #card-head {{ padding: 16px 18px 12px; border-bottom: 1px solid #1a1a3a; flex-shrink: 0; }}
        #card-close {{ position: absolute; top: 12px; right: 14px; background: none; border: none; color: #555577; font-size: 1.3em; cursor: pointer; line-height: 1; padding: 4px; border-radius: 4px; }}
        #card-close:hover {{ color: #ff6b8a; background: #1a1a3e; }}
        #card-type {{ display: flex; align-items: center; gap: 6px; margin-bottom: 6px; }}
        #card-type .ct-dot {{ width: 10px; height: 10px; border-radius: 50%; flex-shrink: 0; }}
        #card-type .ct-label {{ font-size: 0.78em; color: #8888aa; text-transform: uppercase; letter-spacing: 0.5px; font-weight: 500; }}
        #card-name {{ font-size: 1.15em; font-weight: 700; color: #f0f0ff; margin-bottom: 2px; }}
        #card-meta {{ font-size: 0.78em; color: #555577; }}
        #card-body {{ flex: 1; overflow-y: auto; padding: 0; }}
        #card-body::-webkit-scrollbar {{ width: 4px; }}
        #card-body::-webkit-scrollbar-thumb {{ background: #2a2a5a; border-radius: 4px; }}

        /* Connection sections */
        .card-section {{ padding: 12px 18px; border-bottom: 1px solid #141430; }}
        .card-section-title {{ font-size: 0.72em; text-transform: uppercase; letter-spacing: 0.6px; color: #555577; font-weight: 600; margin-bottom: 8px; }}
        .conn-item {{ display: flex; align-items: flex-start; gap: 8px; padding: 6px 0; cursor: pointer; border-radius: 4px; transition: background 0.12s; }}
        .conn-item:hover {{ background: #14142e; margin: 0 -6px; padding: 6px 6px; }}
        .conn-dot {{ width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; margin-top: 5px; }}
        .conn-name {{ color: #c8c8e0; font-size: 0.88em; font-weight: 500; }}
        .conn-rel {{ color: #555577; font-size: 0.78em; margin-top: 1px; line-height: 1.4; }}
        .conn-type {{ color: #444466; font-size: 0.7em; }}

        /* Legend */
        #legend {{ position: fixed; bottom: 12px; left: 12px; background: #0e0e22ee; border: 1px solid #2a2a5a; border-radius: 10px; padding: 10px 14px; font-size: 0.75em; max-height: 240px; overflow-y: auto; z-index: 10; backdrop-filter: blur(8px); min-width: 150px; }}
        #legend .leg-title {{ color: #ff6b8a; font-weight: 600; margin-bottom: 6px; font-size: 0.9em; }}
        #legend .leg-item {{ margin: 2px 0; display: flex; align-items: center; gap: 7px; cursor: pointer; padding: 2px 4px; border-radius: 4px; transition: background 0.15s; }}
        #legend .leg-item:hover {{ background: #1a1a3e; }}
        #legend .leg-dot {{ width: 10px; height: 10px; border-radius: 50%; flex-shrink: 0; }}
        #legend .leg-label {{ color: #aaa; }}
        #legend .leg-count {{ color: #555; margin-left: auto; font-size: 0.9em; }}
        #legend::-webkit-scrollbar {{ width: 4px; }}
        #legend::-webkit-scrollbar-thumb {{ background: #2a2a5a; border-radius: 4px; }}
    </style>
</head>
<body>
    <div id="header">
        <h1>RKnowledge</h1>
        <div class="toolbar">
            <input id="search" type="text" placeholder="Search (/)..." />
            <button class="btn active" id="btnProximity">Proximity</button>
            <button class="btn" id="btnLabels">Labels</button>
            <button class="btn" id="btnPhysics">Freeze</button>
        </div>
        <div class="stats">{} nodes &middot; {} explicit &middot; {} proximity</div>
    </div>
    <div id="graph">
        <div id="loading"><div class="spinner"></div>Loading...</div>
        <div id="error"><p>Failed to load vis-network.</p></div>
    </div>
    <div id="tooltip"></div>
    <div id="card">
        <div id="card-head">
            <button id="card-close">&times;</button>
            <div id="card-type"><span class="ct-dot"></span><span class="ct-label"></span></div>
            <div id="card-name"></div>
            <div id="card-meta"></div>
        </div>
        <div id="card-body"></div>
    </div>
    <div id="legend"></div>

    <script>
        var CDNS = [
            'https://cdnjs.cloudflare.com/ajax/libs/vis-network/9.1.9/vis-network.min.js',
            'https://cdn.jsdelivr.net/npm/vis-network@9.1.9/standalone/umd/vis-network.min.js',
            'https://unpkg.com/vis-network@9.1.9/standalone/umd/vis-network.min.js'
        ];
        function loadScript(u,i){{ if(i>=u.length){{ document.getElementById('loading').style.display='none'; document.getElementById('error').style.display='block'; return; }} var s=document.createElement('script'); s.src=u[i]; s.onload=function(){{ initGraph(); }}; s.onerror=function(){{ loadScript(u,i+1); }}; document.head.appendChild(s); }}
        loadScript(CDNS, 0);

        var graphNodes = {};
        var graphEdges = {};

        function escHtml(s) {{ var d=document.createElement('div'); d.textContent=s; return d.innerHTML; }}
        
        // Curated color palette for better visual distinction
        var colorPalette = [
            {{ bg:'#6366f1', border:'#4f46e5', light:'#818cf8' }}, // Indigo
            {{ bg:'#ec4899', border:'#db2777', light:'#f472b6' }}, // Pink
            {{ bg:'#14b8a6', border:'#0d9488', light:'#2dd4bf' }}, // Teal
            {{ bg:'#f59e0b', border:'#d97706', light:'#fbbf24' }}, // Amber
            {{ bg:'#8b5cf6', border:'#7c3aed', light:'#a78bfa' }}, // Violet
            {{ bg:'#06b6d4', border:'#0891b2', light:'#22d3ee' }}, // Cyan
            {{ bg:'#ef4444', border:'#dc2626', light:'#f87171' }}, // Red
            {{ bg:'#22c55e', border:'#16a34a', light:'#4ade80' }}, // Green
            {{ bg:'#f97316', border:'#ea580c', light:'#fb923c' }}, // Orange
            {{ bg:'#3b82f6', border:'#2563eb', light:'#60a5fa' }}, // Blue
            {{ bg:'#a855f7', border:'#9333ea', light:'#c084fc' }}, // Purple
            {{ bg:'#eab308', border:'#ca8a04', light:'#facc15' }}  // Yellow
        ];
        var typeColorMap = {{}};
        var colorIndex = 0;
        
        function typeToColor(type) {{
            if (!type) type='concept';
            if (!typeColorMap[type]) {{
                typeColorMap[type] = colorPalette[colorIndex % colorPalette.length];
                colorIndex++;
            }}
            return typeColorMap[type];
        }}

        function initGraph() {{
            document.getElementById('loading').style.display='none';

            // Pre-color nodes - labels hidden by default to prevent overlap
            graphNodes.forEach(function(n){{
                var c=typeToColor(n.entityType);
                n.color={{ background:c.bg, border:c.border, highlight:{{ background:c.light, border:c.bg }}, hover:{{ background:c.light, border:c.bg }} }};
                // Truncate long labels
                n.fullLabel = n.label;
                if(n.label.length > 22) n.label = n.label.substring(0, 20) + '…';
                // Start with labels hidden
                n.font={{ color:'transparent', size:11, strokeWidth:0 }};
            }});

            // Style edges - labels off by default
            var showProximity = true;
            graphEdges.forEach(function(e){{
                if(e.isProximity){{
                    e.dashes=[4,4]; e.width=0.4; e.color={{ color:'#1e1e3a', highlight:'#3a3a5a', hover:'#2a2a4a' }};
                    e.font={{ size:0 }}; e.arrows={{to:{{enabled:false}}}};
                }} else {{
                    e.width=Math.max(1, Math.min(3, e.value/3));
                    e.color={{ color:'#4a4a7a', highlight:'#ff6b8a', hover:'#6a6a9a' }};
                    e.font={{ color:'#444466', size:0, strokeWidth:0, align:'middle' }};
                    e.arrows={{to:{{enabled:true,scaleFactor:0.5}}}};
                }}
            }});

            var nodes = new vis.DataSet(graphNodes);
            var edges = new vis.DataSet(graphEdges);
            var container = document.getElementById('graph');
            var options = {{
                nodes: {{ shape:'dot', borderWidth:2, shadow:{{ enabled:true, color:'rgba(0,0,0,0.5)', size:8, x:2, y:3 }},
                    scaling:{{ min:10, max:45, label:{{ enabled:false }} }} }},
                edges: {{ smooth:{{ type:'continuous', roundness:0.2 }}, hoverWidth:2, selectionWidth:2.5 }},
                // Increased spacing to prevent overlap
                physics: {{ forceAtlas2Based:{{ gravitationalConstant:-100, centralGravity:0.004, springLength:300, springConstant:0.06, damping:0.5 }},
                    maxVelocity:40, solver:'forceAtlas2Based', timestep:0.4, stabilization:{{ iterations:400, fit:true }} }},
                interaction: {{ hover:true, tooltipDelay:50, hideEdgesOnDrag:true, hideEdgesOnZoom:true, multiselect:true, zoomSpeed:0.7 }}
            }};
            var network = new vis.Network(container, {{ nodes:nodes, edges:edges }}, options);
            
            // Store original colors for reset
            var nodeColors = {{}};
            graphNodes.forEach(function(n){{ nodeColors[n.id] = n.color; }});
            var highlightActive = false;

            // Neighborhood highlight - show labels only for selected node and neighbors
            function neighbourhoodHighlight(params) {{
                tooltip.style.display='none';
                var allNodes = nodes.get({{ returnType: 'Object' }});
                if (params.nodes.length > 0) {{
                    highlightActive = true;
                    var selectedNode = params.nodes[0];
                    var connectedNodes = network.getConnectedNodes(selectedNode);
                    
                    // Dim all nodes and hide labels
                    for (var nodeId in allNodes) {{
                        allNodes[nodeId].color = 'rgba(80,80,100,0.3)';
                        allNodes[nodeId].font = {{ color: 'transparent', size: 11 }};
                    }}
                    
                    // First-degree neighbors get their color and labels
                    for (var i = 0; i < connectedNodes.length; i++) {{
                        allNodes[connectedNodes[i]].color = nodeColors[connectedNodes[i]];
                        allNodes[connectedNodes[i]].font = {{ color: '#c0c0d0', size: 11 }};
                    }}
                    
                    // Selected node fully highlighted with label
                    allNodes[selectedNode].color = nodeColors[selectedNode];
                    allNodes[selectedNode].font = {{ color: '#ffffff', size: 13, strokeWidth: 3, strokeColor: '#000000' }};
                    
                    nodes.update(Object.values(allNodes));
                    openCard(selectedNode);
                }} else if (highlightActive) {{
                    highlightActive = false;
                    // Reset all nodes
                    for (var nodeId in allNodes) {{
                        allNodes[nodeId].color = nodeColors[nodeId];
                        allNodes[nodeId].font = {{ color: 'transparent', size: 11 }};
                    }}
                    nodes.update(Object.values(allNodes));
                    closeCard();
                }}
            }}
            network.on('click', neighbourhoodHighlight);

            // Show label on hover with stroke for visibility
            network.on('hoverNode', function(p){{
                nodes.update({{ id: p.node, font: {{ color: '#ffffff', size: 13, strokeWidth: 3, strokeColor: '#000000' }} }});
            }});
            network.on('blurNode', function(p){{
                if (!highlightActive) {{
                    nodes.update({{ id: p.node, font: {{ color: 'transparent', size: 11, strokeWidth: 0 }} }});
                }}
            }});

            // ── Hover tooltip (compact preview) ──
            var tooltip = document.getElementById('tooltip');
            network.on('hoverNode', function(p){{
                var n=nodes.get(p.node);
                var tc=typeToColor(n.entityType);
                var conns=network.getConnectedNodes(p.node).length;
                tooltip.innerHTML=
                    '<div class="tt-type"><span class="tt-dot" style="background:'+tc.bg+'"></span>'+escHtml(n.entityType||'concept')+'</div>'+
                    '<div class="tt-name">'+escHtml(n.label)+'</div>'+
                    '<div class="tt-hint">'+conns+' connections &middot; click to expand</div>';
                tooltip.style.display='block';
                var x=p.event.center.x, y=p.event.center.y;
                if(x>window.innerWidth-280) x-=280;
                if(y>window.innerHeight-80) y-=80;
                tooltip.style.left=(x+14)+'px'; tooltip.style.top=(y+14)+'px';
            }});
            network.on('blurNode', function(){{ tooltip.style.display='none'; }});
            network.on('dragStart', function(){{ tooltip.style.display='none'; }});

            // ── Detail card (click) ──
            var card=document.getElementById('card'), cardBody=document.getElementById('card-body');
            var selectedNode=null;

            function openCard(nodeId) {{
                var n=nodes.get(nodeId);
                if(!n) return;
                selectedNode=nodeId;
                var tc=typeToColor(n.entityType);

                // Header: type badge at top, then name
                document.querySelector('#card-type .ct-dot').style.background=tc.bg;
                document.querySelector('#card-type .ct-label').textContent=n.entityType||'concept';
                document.getElementById('card-name').textContent=n.label;

                // Gather connections
                var ce=network.getConnectedEdges(nodeId);
                var explicit=[], proximity=[];
                ce.forEach(function(eId){{
                    var e=edges.get(eId);
                    if(!e) return;
                    var tid=e.from===nodeId?e.to:e.from;
                    var t=nodes.get(tid);
                    if(!t) return;
                    var item={{ id:tid, label:t.label, type:t.entityType||'concept', rel:e.fullLabel||e.label||'related', weight:e.value||1 }};
                    if(e.isProximity) proximity.push(item); else explicit.push(item);
                }});

                // Sort by weight desc
                explicit.sort(function(a,b){{ return b.weight-a.weight; }});
                proximity.sort(function(a,b){{ return b.weight-a.weight; }});

                document.getElementById('card-meta').textContent=explicit.length+' explicit + '+proximity.length+' proximity connections';

                // Build sections
                var html='';

                if(explicit.length>0) {{
                    html+='<div class="card-section"><div class="card-section-title">Explicit Relations ('+explicit.length+')</div>';
                    explicit.forEach(function(c){{
                        var cc=typeToColor(c.type);
                        html+='<div class="conn-item" data-node="'+escHtml(c.id)+'">'+
                            '<span class="conn-dot" style="background:'+cc.bg+'"></span>'+
                            '<div><div class="conn-name">'+escHtml(c.label)+'</div>'+
                            '<div class="conn-rel">'+escHtml(c.rel)+'</div>'+
                            '<div class="conn-type">'+escHtml(c.type)+'</div></div></div>';
                    }});
                    html+='</div>';
                }}

                if(proximity.length>0) {{
                    html+='<div class="card-section"><div class="card-section-title">Contextual Proximity ('+proximity.length+')</div>';
                    proximity.forEach(function(c){{
                        var cc=typeToColor(c.type);
                        html+='<div class="conn-item" data-node="'+escHtml(c.id)+'">'+
                            '<span class="conn-dot" style="background:'+cc.bg+'"></span>'+
                            '<div><div class="conn-name">'+escHtml(c.label)+'</div>'+
                            '<div class="conn-type">'+escHtml(c.type)+'</div></div></div>';
                    }});
                    html+='</div>';
                }}

                if(explicit.length===0 && proximity.length===0) {{
                    html='<div class="card-section" style="color:#444">No connections found.</div>';
                }}

                cardBody.innerHTML=html;

                // Make connection items clickable -- navigate to that node
                cardBody.querySelectorAll('.conn-item').forEach(function(el){{
                    el.addEventListener('click', function(){{
                        var tid=this.getAttribute('data-node');
                        if(tid){{
                            network.focus(tid, {{ scale:1.2, animation:{{ duration:400, easingFunction:'easeInOutQuad' }} }});
                            network.selectNodes([tid]);
                            openCard(tid);
                        }}
                    }});
                }});

                card.classList.add('open');
            }}

            function closeCard() {{
                card.classList.remove('open');
                selectedNode=null;
                network.unselectAll();
            }}

            // Click handler integrated into neighbourhoodHighlight above
            // Card open/close is handled there

            document.getElementById('card-close').addEventListener('click', function(e){{
                e.stopPropagation();
                closeCard();
            }});

            // ── Toggle proximity edges ──
            var btnP=document.getElementById('btnProximity');
            btnP.addEventListener('click', function(){{
                showProximity=!showProximity;
                btnP.classList.toggle('active', showProximity);
                edges.update(graphEdges.filter(function(e){{ return e.isProximity; }}).map(function(e){{
                    return {{ id:e.id, hidden:!showProximity }};
                }}));
            }});

            // ── Toggle edge labels ──
            var showLabels=false, btnL=document.getElementById('btnLabels');
            btnL.addEventListener('click', function(){{
                showLabels=!showLabels;
                btnL.classList.toggle('active', showLabels);
                edges.update(graphEdges.filter(function(e){{ return !e.isProximity; }}).map(function(e){{
                    return {{ id:e.id, font:{{ size: showLabels?8:0 }} }};
                }}));
            }});

            // ── Toggle physics ──
            var physicsOn=true, btnF=document.getElementById('btnPhysics');
            btnF.addEventListener('click', function(){{
                physicsOn=!physicsOn;
                btnF.classList.toggle('active', !physicsOn);
                btnF.textContent=physicsOn?'Freeze':'Unfreeze';
                network.setOptions({{ physics:{{ enabled:physicsOn }} }});
            }});

            // ── Search ──
            var searchInput=document.getElementById('search'), allIds=nodes.getIds();
            var origColors={{}};
            graphNodes.forEach(function(n){{ origColors[n.id]=n.color; }});

            searchInput.addEventListener('input', function(){{
                var q=this.value.toLowerCase().trim();
                if(!q){{
                    nodes.update(allIds.map(function(id){{ return {{ id:id, opacity:1, font:{{ color:'#d0d0e8' }}, color:origColors[id], borderWidth:1.5 }}; }}));
                    return;
                }}
                var match=new Set(), neighbors=new Set();
                allIds.forEach(function(id){{ var n=nodes.get(id); if(n.label.toLowerCase().includes(q)||(n.entityType||'').toLowerCase().includes(q)) match.add(id); }});
                match.forEach(function(id){{ network.getConnectedNodes(id).forEach(function(nid){{ neighbors.add(nid); }}); }});
                nodes.update(allIds.map(function(id){{
                    if(match.has(id)) return {{ id:id, opacity:1, borderWidth:3, font:{{ color:'#ff6b8a' }} }};
                    if(neighbors.has(id)) return {{ id:id, opacity:0.7, borderWidth:2, font:{{ color:'#c8c8e0' }} }};
                    return {{ id:id, opacity:0.08, borderWidth:1, font:{{ color:'#222' }} }};
                }}));
                if(match.size>0) network.focus(Array.from(match)[0], {{ scale:1.1, animation:true }});
            }});

            // ── Legend ──
            var typeCount={{}};
            graphNodes.forEach(function(n){{ var t=n.entityType||'concept'; typeCount[t]=(typeCount[t]||0)+1; }});
            var types=Object.keys(typeCount).sort(function(a,b){{ return typeCount[b]-typeCount[a]; }});
            var leg=document.getElementById('legend');
            leg.innerHTML='<div class="leg-title">Entity Types</div>';
            types.forEach(function(t){{
                var c=typeToColor(t);
                var item=document.createElement('div'); item.className='leg-item';
                item.innerHTML='<span class="leg-dot" style="background:'+c.bg+'"></span><span class="leg-label">'+escHtml(t)+'</span><span class="leg-count">'+typeCount[t]+'</span>';
                item.addEventListener('click', function(){{
                    searchInput.value=t; searchInput.dispatchEvent(new Event('input'));
                }});
                leg.appendChild(item);
            }});

            // ── Keyboard ──
            document.addEventListener('keydown', function(e){{
                if(e.key==='/'&&document.activeElement!==searchInput){{ e.preventDefault(); searchInput.focus(); }}
                if(e.key==='Escape'){{
                    if(card.classList.contains('open')){{ closeCard(); }}
                    else {{ searchInput.value=''; searchInput.dispatchEvent(new Event('input')); searchInput.blur(); }}
                }}
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
