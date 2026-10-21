//! NFR benchmark (PRD §14.1 / AC-20): `cargo run --release --example bench -- /tmp/bench-ws`
//! Builds a reference workspace (10,000 pages, 100,000 database records) and times the operations the
//! NFR targets cover. Prints a Markdown table.
use personal_workspace_lib::application::pages::{self, CreatePage};
use personal_workspace_lib::application::workspace::Workspace;
use personal_workspace_lib::application::databases;
use personal_workspace_lib::infrastructure::{database, search};
use serde_json::{json, Map};
use std::time::Instant;

fn pct(mut v: Vec<f64>, p: f64) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() as f64 - 1.0) * p).round() as usize]
}

fn main() {
    let root = std::path::PathBuf::from(std::env::args().nth(1).expect("usage: bench <empty folder>"));
    let fresh = !root.join("workspace.sqlite").exists();
    let words = ["ประชุม", "ออกแบบ", "รายงาน", "สัปดาห์", "ลูกค้า", "project", "design", "review", "budget", "ทดสอบ", "ระบบ", "แผนงาน"];
    if fresh {
        let ws = Workspace::open(&root).unwrap();
        let c = &ws.conn;
        let t = Instant::now();
        let tx = database::begin(c).unwrap();
        let mut parents: Vec<String> = vec![];
        for i in 0..10_000 {
            let parent = if i % 10 == 0 || parents.is_empty() { None } else { Some(parents[i / 10 % parents.len()].clone()) };
            let p = pages::create(&tx, CreatePage { kind: "page".into(), title: format!("{} {} #{i}", words[i % 12], words[(i * 7) % 12]), parent_id: parent, ..Default::default() }).unwrap();
            if i % 10 == 0 {
                parents.push(p.id.clone());
            }
            let blocks: Vec<_> = (0..8).map(|b| json!({ "id": format!("b{b}"), "type": "paragraph", "content": [{ "type": "text", "text": format!("{} {} {} ข้อความทดสอบบรรทัด {b} ของหน้า {i}", words[(i + b) % 12], words[(i * 3 + b) % 12], words[(i * 5 + b) % 12]), "styles": {} }], "children": [] })).collect();
            pages::save_blocks_raw(&tx, &p.id, &json!(blocks)).unwrap();
        }
        tx.commit().unwrap();
        eprintln!("pages built in {:.1}s", t.elapsed().as_secs_f64());
        let t = Instant::now();
        let db = pages::create(c, CreatePage { kind: "database".into(), title: "Big DB".into(), ..Default::default() }).unwrap();
        let num = databases::add_property(c, &db.id, "Amount", "number").unwrap();
        let info = databases::get(c, &db.id).unwrap();
        let status = info.properties.iter().find(|p| p.ptype == "status").unwrap().clone();
        let opts: Vec<String> = status.config["options"].as_array().unwrap().iter().map(|o| o["id"].as_str().unwrap().to_string()).collect();
        let tx = database::begin(c).unwrap();
        for i in 0..100_000 {
            let mut v = Map::new();
            v.insert(num.id.clone(), json!(i % 977));
            v.insert(status.id.clone(), json!(opts[i % 3]));
            databases::create_record(&tx, &db.id, databases::CreateRecord { title: format!("Record {i} {}", words[i % 12]), values: v, ..Default::default() }).unwrap();
        }
        tx.commit().unwrap();
        eprintln!("records built in {:.1}s", t.elapsed().as_secs_f64());
    }

    let mut rows = vec![];
    let t = Instant::now();
    let ws = Workspace::open(&root).unwrap();
    let tree = pages::list_tree(&ws.conn).unwrap();
    rows.push(("NFR-01 open workspace + load sidebar tree".to_string(), t.elapsed().as_secs_f64() * 1000.0, format!("{} pages in tree", tree.len())));

    let mut times = vec![];
    for i in 0..100 {
        let q = format!("{} {}", words[i % 12], words[(i * 5) % 12]);
        let t = Instant::now();
        let hits = search::search(&ws.conn, &q, &Default::default()).unwrap();
        times.push(t.elapsed().as_secs_f64() * 1000.0);
        let _ = hits.len();
    }
    rows.push(("NFR-02 full-text search, p95 of 100 queries".into(), pct(times.clone(), 0.95), format!("median {:.1} ms", pct(times, 0.5))));

    let mut times = vec![];
    for i in 0..30 {
        let t = Instant::now();
        let p = &tree[i * 37 % tree.len()];
        let _ = pages::open(&ws.conn, &p.id).unwrap();
        times.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    rows.push(("Open a page (blocks + breadcrumb), p95".into(), pct(times, 0.95), "".into()));

    let mut times = vec![];
    let blocks = pages::load_blocks(&ws.conn, &tree[5].id).unwrap();
    for _ in 0..30 {
        let t = Instant::now();
        pages::save_blocks(&ws.conn, &tree[5].id, &json!(blocks)).unwrap();
        times.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    rows.push(("NFR-03 autosave one page (incl. index + links), p95".into(), pct(times, 0.95), "runs off the typing path (debounced)".into()));

    let db = tree.iter().find(|p| p.kind == "database").unwrap();
    let info = databases::get(&ws.conn, &db.id).unwrap();
    let num = info.properties.iter().find(|p| p.name == "Amount").unwrap().id.clone();
    let mut times = vec![];
    for _ in 0..5 {
        let t = Instant::now();
        let r = databases::query(&ws.conn, &db.id, &databases::QuerySpec { limit: Some(200), ..Default::default() }).unwrap();
        times.push(t.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(r.total, 100_000);
    }
    rows.push(("NFR-04 database 100k records: first page of 200".into(), pct(times, 0.5), "median of 5".into()));
    let mut times = vec![];
    for _ in 0..5 {
        let t = Instant::now();
        let r = databases::query(&ws.conn, &db.id, &databases::QuerySpec {
            filter: Some(json!({ "op": "and", "conditions": [{ "propertyId": num, "operator": "gt", "value": 500 }] })),
            sorts: Some(vec![json!({ "propertyId": num, "direction": "desc" })]),
            limit: Some(200), ..Default::default() }).unwrap();
        times.push(t.elapsed().as_secs_f64() * 1000.0);
        let _ = r.total;
    }
    rows.push(("NFR-04 database 100k records: filter + sort".into(), pct(times, 0.5), "median of 5".into()));
    let rec = databases::query(&ws.conn, &db.id, &databases::QuerySpec { limit: Some(20), ..Default::default() }).unwrap().records;
    let mut times = vec![];
    for (i, r) in rec.iter().enumerate() {
        let t = Instant::now();
        databases::set_value(&ws.conn, &r.id, &num, &json!(i)).unwrap();
        times.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    rows.push(("Edit one record value in the 100k database, p95".into(), pct(times, 0.95), "".into()));

    let model = std::process::Command::new("sysctl").args(["-n", "hw.model"]).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let mem = std::process::Command::new("sysctl").args(["-n", "hw.memsize"]).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<u64>().unwrap_or(0) / (1 << 30)).unwrap_or(0);
    let os = std::process::Command::new("sw_vers").arg("-productVersion").output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let size = std::fs::metadata(root.join("workspace.sqlite")).map(|m| m.len() / (1 << 20)).unwrap_or(0);
    println!("Machine: {model}, {mem} GB RAM, macOS {os}. Dataset: 10,000 pages × 8 blocks, 100,000 records; database file {size} MB. Release build.\n");
    println!("| Measurement | Result (ms) | Notes |\n| --- | ---: | --- |");
    for (k, v, n) in rows {
        println!("| {k} | {v:.1} | {n} |");
    }
}
