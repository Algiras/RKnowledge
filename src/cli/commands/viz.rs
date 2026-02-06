use anyhow::{Context, Result};
use console::{style, Emoji};
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
    println!("{}", style(" RKnowledge - Graph Visualization ").bold().reverse());
    println!();

    // Load configuration
    let config = Config::load().context("Failed to load configuration. Run 'rknowledge init' first.")?;

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
    println!("{}File: {}", SPARKLE, style(html_path.display()).cyan().underlined());

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

    let explicit_count = edges.iter().filter(|e| e.relation != "contextual proximity").count();
    let proximity_count = edges.len() - explicit_count;

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>RKnowledge Graph</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{ font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0a0a1a; color: #c8c8e0; }}
        #header {{ padding: 10px 20px; background: linear-gradient(135deg, #12122a 0%, #1a1a3e 100%); border-bottom: 1px solid #2a2a5a; display: flex; align-items: center; gap: 16px; }}
        #header h1 {{ font-size: 1.1em; font-weight: 600; color: #ff6b8a; white-space: nowrap; }}
        .toolbar {{ display: flex; gap: 8px; align-items: center; flex: 1; }}
        #search {{ background: #0e0e22; border: 1px solid #2a2a5a; border-radius: 6px; color: #c8c8e0; padding: 5px 10px; font-size: 0.8em; width: 180px; outline: none; transition: border-color 0.2s; }}
        #search:focus {{ border-color: #ff6b8a; }}
        .btn {{ background: #1a1a3e; border: 1px solid #2a2a5a; border-radius: 6px; color: #8888aa; padding: 5px 10px; font-size: 0.75em; cursor: pointer; transition: all 0.2s; white-space: nowrap; }}
        .btn:hover {{ border-color: #ff6b8a; color: #ff6b8a; }}
        .btn.active {{ background: #2a1a3e; border-color: #ff6b8a; color: #ff6b8a; }}
        .stats {{ font-size: 0.75em; color: #555577; white-space: nowrap; margin-left: auto; }}
        #graph {{ width: 100%; height: calc(100vh - 42px); }}
        #loading {{ position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); text-align: center; color: #555577; }}
        #loading .spinner {{ width: 32px; height: 32px; border: 2px solid #1a1a3e; border-top-color: #ff6b8a; border-radius: 50%; animation: spin 0.7s linear infinite; margin: 0 auto 12px; }}
        @keyframes spin {{ to {{ transform: rotate(360deg); }} }}
        #error {{ display: none; position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); text-align: center; color: #ff6b8a; max-width: 420px; font-size: 0.9em; }}
        #tooltip {{ display: none; position: absolute; background: #14142e; border: 1px solid #2a2a5a; border-radius: 8px; padding: 12px 14px; font-size: 0.82em; max-width: 340px; box-shadow: 0 8px 32px rgba(0,0,0,0.6); pointer-events: none; z-index: 20; backdrop-filter: blur(8px); }}
        #tooltip .tt-name {{ color: #ff6b8a; font-weight: 600; font-size: 1.05em; margin-bottom: 2px; }}
        #tooltip .tt-type {{ font-size: 0.85em; color: #666688; margin-bottom: 8px; display: flex; align-items: center; gap: 5px; }}
        #tooltip .tt-dot {{ width: 8px; height: 8px; border-radius: 50%; display: inline-block; }}
        #tooltip .tt-rels {{ color: #888; line-height: 1.7; }}
        #tooltip .tt-target {{ color: #66ccee; }}
        #tooltip .tt-edge {{ color: #444466; font-size: 0.9em; }}
        #legend {{ position: fixed; bottom: 12px; left: 12px; background: #12122aee; border: 1px solid #2a2a5a; border-radius: 10px; padding: 10px 14px; font-size: 0.75em; max-height: 260px; overflow-y: auto; z-index: 10; backdrop-filter: blur(8px); min-width: 150px; }}
        #legend .leg-title {{ color: #ff6b8a; font-weight: 600; margin-bottom: 6px; font-size: 0.9em; }}
        #legend .leg-item {{ margin: 3px 0; display: flex; align-items: center; gap: 7px; cursor: pointer; padding: 2px 4px; border-radius: 4px; transition: background 0.15s; }}
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
        function typeToColor(type) {{
            if (!type) type='concept';
            var h=0; for(var i=0;i<type.length;i++){{ h=type.charCodeAt(i)+((h<<5)-h); h=h&h; }}
            var hue=((h%360)+360)%360, sat=60+(Math.abs(h>>8)%15), lit=52+(Math.abs(h>>16)%10);
            return {{ bg:'hsl('+hue+','+sat+'%,'+lit+'%)', border:'hsl('+hue+','+sat+'%,'+(lit-12)+'%)', light:'hsl('+hue+','+(sat-10)+'%,'+(lit+15)+'%)' }};
        }}

        function initGraph() {{
            document.getElementById('loading').style.display='none';

            // Pre-color nodes by entity type
            graphNodes.forEach(function(n){{
                var c=typeToColor(n.entityType);
                n.color={{ background:c.bg, border:c.border, highlight:{{ background:c.light, border:c.bg }}, hover:{{ background:c.light, border:c.bg }} }};
                n.font={{ color:'#d0d0e8', size: Math.min(14, Math.max(10, 8+n.value)) }};
            }});

            // Style proximity edges as thin and dashed; explicit edges as solid
            var showProximity = true;
            graphEdges.forEach(function(e){{
                if(e.isProximity){{
                    e.dashes=[4,4]; e.width=0.3; e.color={{ color:'#1a1a3a', highlight:'#3a3a5a', hover:'#2a2a4a' }};
                    e.font={{ size:0 }}; e.arrows={{to:{{enabled:false}}}};
                }} else {{
                    e.width=Math.max(0.8, Math.min(3, e.value/4));
                    e.color={{ color:'#3a3a6a', highlight:'#ff6b8a', hover:'#5a5a8a' }};
                    e.font={{ color:'#444466', size:8, strokeWidth:0, align:'middle' }};
                    e.arrows={{to:{{enabled:true,scaleFactor:0.4}}}};
                }}
            }});

            var nodes = new vis.DataSet(graphNodes);
            var edges = new vis.DataSet(graphEdges);
            var container = document.getElementById('graph');
            var options = {{
                nodes: {{ shape:'dot', borderWidth:1.5, shadow:{{ enabled:true, color:'rgba(0,0,0,0.4)', size:6, x:2, y:2 }},
                    scaling:{{ min:6, max:35, label:{{ enabled:true, min:9, max:18 }} }} }},
                edges: {{ smooth:{{ type:'continuous', roundness:0.15 }}, hoverWidth:1.5, selectionWidth:2 }},
                physics: {{ forceAtlas2Based:{{ gravitationalConstant:-40, centralGravity:0.006, springLength:160, springConstant:0.12, damping:0.45 }},
                    maxVelocity:60, solver:'forceAtlas2Based', timestep:0.35, stabilization:{{ iterations:250, fit:true }} }},
                interaction: {{ hover:true, tooltipDelay:80, hideEdgesOnDrag:true, multiselect:true, zoomSpeed:0.8 }}
            }};
            var network = new vis.Network(container, {{ nodes:nodes, edges:edges }}, options);

            // ── Tooltip ──
            var tooltip = document.getElementById('tooltip');
            network.on('hoverNode', function(p){{
                var n=nodes.get(p.node), ce=network.getConnectedEdges(p.node);
                var tc=typeToColor(n.entityType);
                var html='<div class="tt-name">'+escHtml(n.label)+'</div>';
                html+='<div class="tt-type"><span class="tt-dot" style="background:'+tc.bg+'"></span>'+escHtml(n.entityType||'')+'</div>';
                // Show explicit relations first, then proximity
                var explicit=[], proximity=[];
                ce.forEach(function(eId){{
                    var e=edges.get(eId), tid=e.from===p.node?e.to:e.from, t=nodes.get(tid);
                    if(t){{ (e.isProximity?proximity:explicit).push({{ label:t.label, rel:e.fullLabel||e.label||'related' }}); }}
                }});
                html+='<div class="tt-rels">';
                explicit.slice(0,6).forEach(function(r){{ html+='<span class="tt-target">'+escHtml(r.label)+'</span> <span class="tt-edge">'+escHtml(r.rel)+'</span><br>'; }});
                if(proximity.length>0 && explicit.length<6) {{
                    var left=Math.min(proximity.length, 4);
                    proximity.slice(0,left).forEach(function(r){{ html+='<span style="color:#556">'+escHtml(r.label)+'</span><br>'; }});
                }}
                var total=explicit.length+proximity.length;
                if(total>10) html+='<span style="color:#445">+'+(total-10)+' more</span>';
                html+='</div>';
                tooltip.innerHTML=html; tooltip.style.display='block';
                var x=p.event.center.x, y=p.event.center.y;
                if(x>window.innerWidth-360) x-=360;
                tooltip.style.left=(x+14)+'px'; tooltip.style.top=(y+14)+'px';
            }});
            network.on('blurNode', function(){{ tooltip.style.display='none'; }});
            network.on('dragStart', function(){{ tooltip.style.display='none'; }});

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
            // Store original colors for reset
            var origColors={{}};
            graphNodes.forEach(function(n){{ origColors[n.id]=n.color; }});

            searchInput.addEventListener('input', function(){{
                var q=this.value.toLowerCase().trim();
                if(!q){{
                    nodes.update(allIds.map(function(id){{ var n=nodes.get(id); return {{ id:id, opacity:1, font:{{ color:'#d0d0e8' }}, color:origColors[id], borderWidth:1.5 }}; }}));
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
                if(e.key==='Escape'){{ searchInput.value=''; searchInput.dispatchEvent(new Event('input')); searchInput.blur(); }}
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
