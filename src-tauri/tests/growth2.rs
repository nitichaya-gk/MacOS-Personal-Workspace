//! Growth PRD phases B, C, F: learning prerequisites, recurring bills, CSV import, encrypted backups, orphans.

use personal_workspace_lib::application::workspace::Workspace;
use personal_workspace_lib::application::{finance, goals, tasks};
use personal_workspace_lib::infrastructure::backup;
use serde_json::json;

fn ws() -> (tempfile::TempDir, Workspace) {
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::open(&dir.path().join("ws")).unwrap();
    (dir, ws)
}

#[test]
fn recurring_bills_post_and_advance() {
    let (_d, ws) = ws();
    let c = &ws.conn;
    let acc = finance::create_account(c, &json!({ "name": "Bank", "openingBalance": "1000" })).unwrap()["id"].as_str().unwrap().to_string();
    let r = finance::save_recurring(c, None, &json!({ "title": "ค่าอินเทอร์เน็ต", "type": "expense", "accountId": acc, "amount": "599", "frequency": "monthly", "nextDate": "2026-01-31", "categoryId": "cat-housing" })).unwrap();
    let id = r["id"].as_str().unwrap();
    let out = finance::settle_recurring(c, id, true).unwrap();
    assert_eq!(out["nextDate"], "2026-02-28", "month-end clamps");
    finance::settle_recurring(c, id, false).unwrap();
    let accts = finance::accounts(c, false).unwrap();
    assert_eq!(accts[0]["balanceMinor"], 100000 - 59900, "only the posted occurrence counts");
}

#[test]
fn csv_import_maps_columns_and_skips_duplicates() {
    let (_d, ws) = ws();
    let c = &ws.conn;
    let acc = finance::create_account(c, &json!({ "name": "Bank" })).unwrap()["id"].as_str().unwrap().to_string();
    let csv = "วันที่,รายการ,จำนวนเงิน\n21/10/2569,ค่ากาแฟ,-65.00\n22/10/2569,เงินเดือน,\"15,000.00\"\nxx,bad,1\n";
    let map = json!({ "date": "วันที่", "amount": "จำนวนเงิน", "description": "รายการ" });
    let dry = finance::import_csv(c, &acc, csv, &map, true).unwrap();
    assert_eq!(dry["toAdd"], 2);
    assert_eq!(dry["errors"].as_array().unwrap().len(), 1);
    assert_eq!(dry["preview"][0]["date"], "2026-10-21", "Buddhist-era year converted");
    assert!(finance::list_transactions(c, None, None, None, 10).unwrap().is_empty(), "dry run writes nothing");
    finance::import_csv(c, &acc, csv, &map, false).unwrap();
    let again = finance::import_csv(c, &acc, csv, &map, false).unwrap();
    assert_eq!(again["toAdd"], 0);
    assert_eq!(again["duplicates"], 2);
    let m = finance::month_summary(c, "2026-10").unwrap();
    assert_eq!(m["currencies"][0]["incomeMinor"], 1500000);
    assert_eq!(m["currencies"][0]["expenseMinor"], 6500);
}

#[test]
fn transaction_drafts_need_confirmation() {
    let (_d, ws) = ws();
    let c = &ws.conn;
    let acc = finance::create_account(c, &json!({ "name": "Cash" })).unwrap()["id"].as_str().unwrap().to_string();
    let d = finance::draft_transaction(c, &json!({ "type": "expense", "amount": "120", "merchantOrSource": "ร้านหนังสือ" }), "mcp:test").unwrap();
    assert!(finance::list_transactions(c, None, None, None, 10).unwrap().is_empty());
    finance::resolve_transaction_draft(c, d["draftId"].as_str().unwrap(), Some(&acc)).unwrap();
    assert_eq!(finance::list_transactions(c, None, None, None, 10).unwrap().len(), 1);
    assert!(finance::transaction_drafts(c).unwrap().is_empty());
}

#[test]
fn encrypted_backup_round_trip_and_wrong_password() {
    let (dir, ws) = ws();
    let dest = dir.path().join("backup.pwbackup");
    backup::export_encrypted(&ws.conn, &ws.root, &ws.manifest().unwrap(), &dest, "correct horse").unwrap();
    let raw = std::fs::read(&dest).unwrap();
    assert!(backup::is_encrypted(&raw));
    assert!(!raw.windows(15).any(|w| w == b"SQLite format 3"), "contents are not readable");
    assert!(backup::restore_encrypted(&dest, &dir.path().join("r1"), "wrong pass").unwrap_err().to_string().contains("wrong password"));
    let r = backup::restore_encrypted(&dest, &dir.path().join("r2"), "correct horse").unwrap();
    assert_eq!(r["integrity"]["ok"], true);
    assert!(backup::encrypt(b"x", "short").is_err());
}

#[test]
fn orphan_detector_finds_unconnected_items() {
    let (_d, ws) = ws();
    let c = &ws.conn;
    tasks::create(c, &json!({ "title": "ลอย ๆ" })).unwrap();
    goals::create(c, &json!({ "title": "ไม่มีเกณฑ์", "domain": "personal", "status": "active" })).unwrap();
    let o = goals::orphans(c).unwrap();
    assert_eq!(o["tasksWithoutGoalOrProject"].as_array().unwrap().len(), 1);
    assert_eq!(o["goalsWithoutRequirements"].as_array().unwrap().len(), 1);
    assert_eq!(o["goalsWithoutSuccessDefinition"].as_array().unwrap().len(), 1);
}
