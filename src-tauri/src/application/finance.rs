//! Manual personal finance. All amounts are integer minor units (satang for THB) — no floating point.
//! Transfers are stored as a signed pair sharing `transfer_group_id` and never count as income or expense.
//! Totals are always per currency; nothing is converted between currencies.

use super::tasks;
use crate::domain::{new_id, now};
use crate::error::{invalid, AppError, Result};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::{json, Value};

fn audit(conn: &Connection, entity: &str, id: &str, action: &str, before: Option<&Value>, after: Option<&Value>) -> Result<()> {
    conn.execute(
        "INSERT INTO audit_events(id, entity_type, entity_id, action, before_json, after_json, at) VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![new_id(), entity, id, action, before.map(|v| v.to_string()), after.map(|v| v.to_string()), now()],
    )?;
    Ok(())
}

fn amount(p: &Value, key: &str) -> Result<i64> {
    match p.get(key) {
        Some(Value::Number(n)) => n.as_i64().ok_or_else(|| AppError::Invalid(format!("{key} must be a whole number of minor units"))),
        Some(Value::String(s)) => crate::domain::growth::parse_money(s).ok_or_else(|| AppError::Invalid(format!("{key}: enter an amount like 1,234.50"))),
        _ => invalid(format!("{key} is required")),
    }
}

fn valid_currency(c: &str) -> bool {
    c.len() == 3 && c.chars().all(|x| x.is_ascii_uppercase())
}

// ---------- accounts & categories ----------

pub fn accounts(conn: &Connection, include_archived: bool) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.name, a.type, a.currency, a.opening_balance_minor, a.archived_at,
                a.opening_balance_minor + COALESCE((SELECT SUM(CASE t.type WHEN 'expense' THEN -t.amount_minor ELSE t.amount_minor END)
                  FROM transactions t WHERE t.account_id=a.id AND t.deleted_at IS NULL), 0)
         FROM financial_accounts a WHERE (?1=1 OR a.archived_at IS NULL) ORDER BY a.created_at",
    )?;
    let rows = stmt.query_map([include_archived as i64], |r| {
        Ok(json!({ "id": r.get::<_, String>(0)?, "name": r.get::<_, String>(1)?, "type": r.get::<_, String>(2)?, "currency": r.get::<_, String>(3)?,
                   "openingBalanceMinor": r.get::<_, i64>(4)?, "archivedAt": r.get::<_, Option<String>>(5)?, "balanceMinor": r.get::<_, i64>(6)? }))
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

pub fn create_account(conn: &Connection, p: &Value) -> Result<Value> {
    let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("").trim();
    if name.is_empty() {
        return invalid("account name is required");
    }
    let currency = p.get("currency").and_then(|v| v.as_str()).unwrap_or("THB").to_uppercase();
    if !valid_currency(&currency) {
        return invalid("currency must be a 3-letter code such as THB");
    }
    let atype = p.get("type").and_then(|v| v.as_str()).unwrap_or("cash");
    if !matches!(atype, "cash" | "bank" | "savings" | "credit" | "ewallet" | "other") {
        return invalid("unknown account type");
    }
    let opening = if p.get("openingBalance").is_some() { amount(p, "openingBalance")? } else { 0 };
    let id = new_id();
    conn.execute(
        "INSERT INTO financial_accounts(id, name, type, currency, opening_balance_minor, created_at) VALUES (?1,?2,?3,?4,?5,?6)",
        params![id, name, atype, currency, opening, now()],
    )?;
    audit(conn, "account", &id, "create", None, Some(p))?;
    accounts(conn, true)?.into_iter().find(|a| a["id"] == id.as_str()).ok_or_else(|| AppError::NotFound("account".into()))
}

pub fn update_account(conn: &Connection, id: &str, p: &Value) -> Result<()> {
    if let Some(n) = p.get("name").and_then(|v| v.as_str()) {
        if n.trim().is_empty() {
            return invalid("account name is required");
        }
        conn.execute("UPDATE financial_accounts SET name=?2 WHERE id=?1", params![id, n.trim()])?;
    }
    if let Some(a) = p.get("archived").and_then(|v| v.as_bool()) {
        conn.execute("UPDATE financial_accounts SET archived_at=?2 WHERE id=?1", params![id, a.then(now)])?;
    }
    if p.get("openingBalance").is_some() {
        conn.execute("UPDATE financial_accounts SET opening_balance_minor=?2 WHERE id=?1", params![id, amount(p, "openingBalance")?])?;
    }
    audit(conn, "account", id, "update", None, Some(p))
}

pub fn categories(conn: &Connection) -> Result<Vec<Value>> {
    let rows = conn
        .prepare("SELECT id, name, kind, color FROM financial_categories ORDER BY kind DESC, position, name")?
        .query_map([], |r| Ok(json!({ "id": r.get::<_, String>(0)?, "name": r.get::<_, String>(1)?, "kind": r.get::<_, String>(2)?, "color": r.get::<_, String>(3)? })))?
        .collect::<std::result::Result<_, _>>()?;
    Ok(rows)
}

pub fn create_category(conn: &Connection, name: &str, kind: &str) -> Result<Value> {
    if name.trim().is_empty() || !matches!(kind, "income" | "expense") {
        return invalid("category needs a name and kind income/expense");
    }
    let id = new_id();
    conn.execute("INSERT INTO financial_categories(id, name, kind, position) VALUES (?1,?2,?3,100)", params![id, name.trim(), kind])?;
    Ok(json!({ "id": id, "name": name.trim(), "kind": kind, "color": "gray" }))
}

// ---------- transactions ----------

fn tx_row(r: &Row) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": r.get::<_, String>(0)?, "accountId": r.get::<_, String>(1)?, "type": r.get::<_, String>(2)?, "amountMinor": r.get::<_, i64>(3)?,
        "currency": r.get::<_, String>(4)?, "categoryId": r.get::<_, Option<String>>(5)?, "occurredOn": r.get::<_, String>(6)?,
        "merchantOrSource": r.get::<_, String>(7)?, "note": r.get::<_, String>(8)?, "linkedGoalId": r.get::<_, Option<String>>(9)?,
        "transferGroupId": r.get::<_, Option<String>>(10)?, "accountName": r.get::<_, String>(11)?, "categoryName": r.get::<_, Option<String>>(12)?,
        "goalTitle": r.get::<_, Option<String>>(13)?,
    }))
}
const TX_SELECT: &str = "SELECT t.id, t.account_id, t.type, t.amount_minor, t.currency, t.category_id, t.occurred_on, t.merchant_or_source, t.note,
    t.linked_goal_id, t.transfer_group_id, a.name, c.name, g.title
    FROM transactions t JOIN financial_accounts a ON a.id=t.account_id LEFT JOIN financial_categories c ON c.id=t.category_id LEFT JOIN goals g ON g.id=t.linked_goal_id";

fn get_tx(conn: &Connection, id: &str) -> Result<Value> {
    conn.query_row(&format!("{TX_SELECT} WHERE t.id=?1"), [id], tx_row).optional()?.ok_or_else(|| AppError::NotFound("transaction".into()))
}

fn account_currency(conn: &Connection, id: &str) -> Result<String> {
    conn.query_row("SELECT currency FROM financial_accounts WHERE id=?1", [id], |r| r.get(0))
        .optional()?
        .ok_or_else(|| AppError::NotFound("account".into()))
}

/// income / expense: {accountId, amount, categoryId, occurredOn, …}; transfer: {fromAccountId, toAccountId, amount, …}.
pub fn add_transaction(conn: &Connection, p: &Value) -> Result<Value> {
    let ttype = p.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let amt = amount(p, "amount")?;
    if amt <= 0 {
        return invalid("amount must be greater than 0");
    }
    let date = p.get("occurredOn").and_then(|v| v.as_str()).map(String::from).unwrap_or_else(|| tasks::today(conn));
    if crate::domain::growth::date(&date).is_none() {
        return invalid("date must be YYYY-MM-DD");
    }
    let s = |k: &str| p.get(k).and_then(|v| v.as_str()).filter(|v| !v.is_empty()).map(String::from);
    let ts = now();
    let tx = crate::infrastructure::database::begin(conn)?;
    let id = new_id();
    match ttype {
        "income" | "expense" => {
            let acc = s("accountId").ok_or_else(|| AppError::Invalid("choose an account".into()))?;
            let cur = account_currency(&tx, &acc)?;
            if let Some(c) = s("categoryId") {
                let kind: String = tx.query_row("SELECT kind FROM financial_categories WHERE id=?1", [&c], |r| r.get(0))?;
                if kind != ttype {
                    return invalid(format!("that category is for {kind}"));
                }
            }
            tx.execute(
                "INSERT INTO transactions(id, account_id, type, amount_minor, currency, category_id, occurred_on, merchant_or_source, note, linked_goal_id, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?11)",
                params![id, acc, ttype, amt, cur, s("categoryId"), date, s("merchantOrSource").unwrap_or_default(), s("note").unwrap_or_default(), s("linkedGoalId"), ts],
            )?;
        }
        "transfer" => {
            let from = s("fromAccountId").ok_or_else(|| AppError::Invalid("choose the account to move money from".into()))?;
            let to = s("toAccountId").ok_or_else(|| AppError::Invalid("choose the account to move money to".into()))?;
            if from == to {
                return invalid("choose two different accounts");
            }
            let (cf, ct) = (account_currency(&tx, &from)?, account_currency(&tx, &to)?);
            if cf != ct {
                return invalid("transfers between currencies are not supported — record an expense and an income with the rate in the note");
            }
            let group = new_id();
            for (tid, acc, signed) in [(id.clone(), from, -amt), (new_id(), to, amt)] {
                tx.execute(
                    "INSERT INTO transactions(id, account_id, type, amount_minor, currency, occurred_on, note, transfer_group_id, created_at, updated_at)
                     VALUES (?1,?2,'transfer',?3,?4,?5,?6,?7,?8,?8)",
                    params![tid, acc, signed, cf, date, s("note").unwrap_or_default(), group, ts],
                )?;
            }
        }
        _ => return invalid("type must be income, expense or transfer"),
    }
    let out = get_tx(&tx, &id)?;
    audit(&tx, "transaction", &id, "create", None, Some(&out))?;
    tx.commit()?;
    Ok(out)
}

pub fn update_transaction(conn: &Connection, id: &str, p: &Value) -> Result<Value> {
    let before = get_tx(conn, id)?;
    if before["type"] == "transfer" {
        return invalid("delete the transfer and record it again to change it");
    }
    if p.get("amount").is_some() {
        let a = amount(p, "amount")?;
        if a <= 0 {
            return invalid("amount must be greater than 0");
        }
        conn.execute("UPDATE transactions SET amount_minor=?2 WHERE id=?1", params![id, a])?;
    }
    for (col, key) in [("category_id", "categoryId"), ("occurred_on", "occurredOn"), ("merchant_or_source", "merchantOrSource"), ("note", "note"), ("linked_goal_id", "linkedGoalId")] {
        if let Some(v) = p.get(key) {
            let v = v.as_str().filter(|s| !s.is_empty());
            if col == "occurred_on" && v.and_then(crate::domain::growth::date).is_none() {
                return invalid("date must be YYYY-MM-DD");
            }
            let v = if matches!(col, "merchant_or_source" | "note") { Some(v.unwrap_or("")) } else { v };
            conn.execute(&format!("UPDATE transactions SET {col}=?2 WHERE id=?1"), params![id, v])?;
        }
    }
    conn.execute("UPDATE transactions SET updated_at=?2 WHERE id=?1", params![id, now()])?;
    let after = get_tx(conn, id)?;
    audit(conn, "transaction", id, "update", Some(&before), Some(&after))?;
    Ok(after)
}

/// Soft delete (restorable). Deleting either half of a transfer deletes both halves.
pub fn delete_transaction(conn: &Connection, id: &str) -> Result<()> {
    let before = get_tx(conn, id)?;
    let ts = now();
    match before["transferGroupId"].as_str() {
        Some(g) => conn.execute("UPDATE transactions SET deleted_at=?2 WHERE transfer_group_id=?1", params![g, ts])?,
        None => conn.execute("UPDATE transactions SET deleted_at=?2 WHERE id=?1", params![id, ts])?,
    };
    audit(conn, "transaction", id, "delete", Some(&before), None)
}

pub fn restore_transaction(conn: &Connection, id: &str) -> Result<()> {
    let before = get_tx(conn, id)?;
    match before["transferGroupId"].as_str() {
        Some(g) => conn.execute("UPDATE transactions SET deleted_at=NULL WHERE transfer_group_id=?1", [g])?,
        None => conn.execute("UPDATE transactions SET deleted_at=NULL WHERE id=?1", [id])?,
    };
    audit(conn, "transaction", id, "restore", None, Some(&before))
}

pub fn list_transactions(conn: &Connection, month: Option<&str>, account_id: Option<&str>, goal_id: Option<&str>, limit: usize) -> Result<Vec<Value>> {
    let rows = conn
        .prepare(&format!(
            "{TX_SELECT} WHERE t.deleted_at IS NULL AND (?1 IS NULL OR substr(t.occurred_on,1,7)=?1) AND (?2 IS NULL OR t.account_id=?2)
             AND (?3 IS NULL OR t.linked_goal_id=?3) ORDER BY t.occurred_on DESC, t.created_at DESC LIMIT ?4"
        ))?
        .query_map(params![month, account_id, goal_id, limit as i64], tx_row)?
        .collect::<std::result::Result<_, _>>()?;
    Ok(rows)
}

// ---------- budgets & savings ----------

pub fn set_budget(conn: &Connection, category_id: &str, p: &Value) -> Result<()> {
    let currency = p.get("currency").and_then(|v| v.as_str()).unwrap_or("THB").to_uppercase();
    if p.get("limit").map(|v| v.is_null()).unwrap_or(true) {
        conn.execute("DELETE FROM budgets WHERE category_id=?1 AND currency=?2", params![category_id, currency])?;
        return Ok(());
    }
    let limit = amount(p, "limit")?;
    if limit <= 0 {
        return invalid("budget must be greater than 0");
    }
    conn.execute(
        "INSERT INTO budgets(id, category_id, limit_minor, currency, goal_id, created_at) VALUES (?1,?2,?3,?4,?5,?6)
         ON CONFLICT(category_id, currency) DO UPDATE SET limit_minor=excluded.limit_minor, goal_id=excluded.goal_id",
        params![new_id(), category_id, limit, currency, p.get("goalId").and_then(|v| v.as_str()), now()],
    )?;
    Ok(())
}

pub fn savings_goals(conn: &Connection, goal_id: Option<&str>) -> Result<Vec<Value>> {
    let rows = conn
        .prepare(
            "SELECT s.id, s.title, s.target_minor, s.currency, s.target_date, s.goal_id, g.title,
                    COALESCE((SELECT SUM(amount_minor) FROM savings_allocations a WHERE a.savings_goal_id=s.id),0)
             FROM savings_goals s LEFT JOIN goals g ON g.id=s.goal_id WHERE s.deleted_at IS NULL AND (?1 IS NULL OR s.goal_id=?1) ORDER BY s.target_date IS NULL, s.target_date",
        )?
        .query_map([goal_id], |r| {
            let target: i64 = r.get(2)?;
            let saved: i64 = r.get(7)?;
            Ok(json!({ "id": r.get::<_, String>(0)?, "title": r.get::<_, String>(1)?, "targetMinor": target, "currency": r.get::<_, String>(3)?,
                       "targetDate": r.get::<_, Option<String>>(4)?, "goalId": r.get::<_, Option<String>>(5)?, "goalTitle": r.get::<_, Option<String>>(6)?,
                       "savedMinor": saved, "progress": (saved as f64 / target as f64).clamp(0.0, 1.0) }))
        })?
        .collect::<std::result::Result<_, _>>()?;
    Ok(rows)
}

pub fn create_savings_goal(conn: &Connection, p: &Value) -> Result<Value> {
    let title = p.get("title").and_then(|v| v.as_str()).unwrap_or("").trim();
    if title.is_empty() {
        return invalid("savings goal needs a name");
    }
    let target = amount(p, "target")?;
    if target <= 0 {
        return invalid("target must be greater than 0");
    }
    let currency = p.get("currency").and_then(|v| v.as_str()).unwrap_or("THB").to_uppercase();
    let id = new_id();
    conn.execute(
        "INSERT INTO savings_goals(id, title, target_minor, currency, target_date, goal_id, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![id, title, target, currency, p.get("targetDate").and_then(|v| v.as_str()).filter(|s| !s.is_empty()), p.get("goalId").and_then(|v| v.as_str()).filter(|s| !s.is_empty()), now()],
    )?;
    audit(conn, "savings_goal", &id, "create", None, Some(p))?;
    Ok(json!({ "id": id }))
}

pub fn delete_savings_goal(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("UPDATE savings_goals SET deleted_at=?2 WHERE id=?1", params![id, now()])?;
    audit(conn, "savings_goal", id, "delete", None, None)
}

/// Earmarks existing money for a savings goal (negative to release). It does not create money:
/// allocations can never exceed the cash available in accounts of that currency.
pub fn allocate(conn: &Connection, savings_goal_id: &str, p: &Value) -> Result<Value> {
    let amt = amount(p, "amount")?;
    let currency: String = conn.query_row("SELECT currency FROM savings_goals WHERE id=?1", [savings_goal_id], |r| r.get(0))?;
    let s = summary_for_currency(conn, &currency)?;
    if amt > 0 && amt > s.0 - s.1 {
        return invalid(format!("only {} {currency} is unallocated", crate::domain::growth::format_money(s.0 - s.1)));
    }
    let saved: i64 = conn.query_row("SELECT COALESCE(SUM(amount_minor),0) FROM savings_allocations WHERE savings_goal_id=?1", [savings_goal_id], |r| r.get(0))?;
    if saved + amt < 0 {
        return invalid("cannot release more than is allocated");
    }
    conn.execute(
        "INSERT INTO savings_allocations(id, savings_goal_id, amount_minor, allocated_on, note, created_at) VALUES (?1,?2,?3,?4,?5,?6)",
        params![new_id(), savings_goal_id, amt, tasks::today(conn), p.get("note").and_then(|v| v.as_str()).unwrap_or(""), now()],
    )?;
    audit(conn, "savings_goal", savings_goal_id, "allocate", None, Some(p))?;
    Ok(json!({ "ok": true }))
}

/// (total balance, allocated savings) for one currency.
fn summary_for_currency(conn: &Connection, currency: &str) -> Result<(i64, i64)> {
    let balance: i64 = accounts(conn, false)?.iter().filter(|a| a["currency"] == currency).map(|a| a["balanceMinor"].as_i64().unwrap_or(0)).sum();
    let allocated: i64 = conn.query_row(
        "SELECT COALESCE(SUM(a.amount_minor),0) FROM savings_allocations a JOIN savings_goals s ON s.id=a.savings_goal_id WHERE s.deleted_at IS NULL AND s.currency=?1",
        [currency],
        |r| r.get(0),
    )?;
    Ok((balance, allocated))
}

/// Monthly overview per currency: income, expense (transfers excluded), net flow, by category, budgets, cash vs allocated.
pub fn month_summary(conn: &Connection, month: &str) -> Result<Value> {
    if month.len() != 7 || crate::domain::growth::date(&format!("{month}-01")).is_none() {
        return invalid("month must be YYYY-MM");
    }
    let currencies: Vec<String> = conn
        .prepare("SELECT DISTINCT currency FROM financial_accounts WHERE archived_at IS NULL UNION SELECT DISTINCT currency FROM budgets ORDER BY 1")?
        .query_map([], |r| r.get(0))?
        .collect::<std::result::Result<_, _>>()?;
    let mut out = vec![];
    for cur in currencies {
        let sum = |t: &str| -> Result<i64> {
            Ok(conn.query_row(
                "SELECT COALESCE(SUM(amount_minor),0) FROM transactions WHERE deleted_at IS NULL AND type=?1 AND currency=?2 AND substr(occurred_on,1,7)=?3",
                params![t, cur, month],
                |r| r.get(0),
            )?)
        };
        let (income, expense) = (sum("income")?, sum("expense")?);
        let by_category: Vec<Value> = conn
            .prepare(
                "SELECT c.id, c.name, c.kind, c.color, COALESCE(SUM(t.amount_minor),0) FROM transactions t LEFT JOIN financial_categories c ON c.id=t.category_id
                 WHERE t.deleted_at IS NULL AND t.type<>'transfer' AND t.currency=?1 AND substr(t.occurred_on,1,7)=?2 GROUP BY c.id ORDER BY 5 DESC",
            )?
            .query_map(params![cur, month], |r| Ok(json!({ "categoryId": r.get::<_, Option<String>>(0)?, "name": r.get::<_, Option<String>>(1)?, "kind": r.get::<_, Option<String>>(2)?, "color": r.get::<_, Option<String>>(3)?, "totalMinor": r.get::<_, i64>(4)? })))?
            .collect::<std::result::Result<_, _>>()?;
        let budgets: Vec<Value> = conn
            .prepare(
                "SELECT b.category_id, c.name, b.limit_minor, b.goal_id,
                        COALESCE((SELECT SUM(t.amount_minor) FROM transactions t WHERE t.deleted_at IS NULL AND t.type='expense' AND t.category_id=b.category_id AND t.currency=b.currency AND substr(t.occurred_on,1,7)=?2),0)
                 FROM budgets b JOIN financial_categories c ON c.id=b.category_id WHERE b.currency=?1 ORDER BY c.position",
            )?
            .query_map(params![cur, month], |r| {
                let limit: i64 = r.get(2)?;
                let spent: i64 = r.get(4)?;
                Ok(json!({ "categoryId": r.get::<_, String>(0)?, "name": r.get::<_, String>(1)?, "limitMinor": limit, "goalId": r.get::<_, Option<String>>(3)?,
                           "spentMinor": spent, "remainingMinor": limit - spent, "over": spent > limit }))
            })?
            .collect::<std::result::Result<_, _>>()?;
        let (balance, allocated) = summary_for_currency(conn, &cur)?;
        out.push(json!({
            "currency": cur, "incomeMinor": income, "expenseMinor": expense, "netFlowMinor": income - expense,
            "byCategory": by_category, "budgets": budgets,
            "totalBalanceMinor": balance, "allocatedSavingsMinor": allocated, "availableCashMinor": balance - allocated,
        }));
    }
    Ok(json!({ "month": month, "currencies": out }))
}

/// Finance linked to a goal: expenses tagged to it, its savings goals and budgets.
pub fn goal_finance(conn: &Connection, goal_id: &str) -> Result<Value> {
    let spent: Vec<Value> = conn
        .prepare("SELECT currency, COALESCE(SUM(amount_minor),0) FROM transactions WHERE deleted_at IS NULL AND type='expense' AND linked_goal_id=?1 GROUP BY currency")?
        .query_map([goal_id], |r| Ok(json!({ "currency": r.get::<_, String>(0)?, "spentMinor": r.get::<_, i64>(1)? })))?
        .collect::<std::result::Result<_, _>>()?;
    Ok(json!({ "spent": spent, "savingsGoals": savings_goals(conn, Some(goal_id))?, "transactions": list_transactions(conn, None, None, Some(goal_id), 50)? }))
}

pub fn audit_trail(conn: &Connection, limit: usize) -> Result<Vec<Value>> {
    let rows = conn
        .prepare("SELECT entity_type, entity_id, action, at FROM audit_events ORDER BY at DESC LIMIT ?1")?
        .query_map([limit as i64], |r| Ok(json!({ "entityType": r.get::<_, String>(0)?, "entityId": r.get::<_, String>(1)?, "action": r.get::<_, String>(2)?, "at": r.get::<_, String>(3)? })))?
        .collect::<std::result::Result<_, _>>()?;
    Ok(rows)
}

// ---------- recurring bills / income ----------

fn advance(date: &str, freq: &str) -> Result<String> {
    let d = crate::domain::growth::date(date).ok_or_else(|| AppError::Invalid("bad date".into()))?;
    use chrono::Datelike;
    let next = match freq {
        "weekly" => d + chrono::Duration::days(7),
        "yearly" => d.with_year(d.year() + 1).unwrap_or(d + chrono::Duration::days(365)),
        _ => {
            let (y, m) = if d.month() == 12 { (d.year() + 1, 1) } else { (d.year(), d.month() + 1) };
            let mut day = d.day();
            loop {
                if let Some(x) = chrono::NaiveDate::from_ymd_opt(y, m, day) {
                    break x;
                }
                day -= 1;
            }
        }
    };
    Ok(next.to_string())
}

pub fn recurring(conn: &Connection) -> Result<Vec<Value>> {
    let today = tasks::today(conn);
    let rows = conn
        .prepare(
            "SELECT r.id, r.title, r.type, r.account_id, a.name, r.category_id, c.name, r.amount_minor, a.currency, r.frequency, r.next_date, r.end_date, r.linked_goal_id, r.active
             FROM recurring_transactions r JOIN financial_accounts a ON a.id=r.account_id LEFT JOIN financial_categories c ON c.id=r.category_id ORDER BY r.next_date",
        )?
        .query_map([], |r| {
            let next: String = r.get(10)?;
            Ok(json!({ "id": r.get::<_, String>(0)?, "title": r.get::<_, String>(1)?, "type": r.get::<_, String>(2)?, "accountId": r.get::<_, String>(3)?,
                       "accountName": r.get::<_, String>(4)?, "categoryId": r.get::<_, Option<String>>(5)?, "categoryName": r.get::<_, Option<String>>(6)?,
                       "amountMinor": r.get::<_, i64>(7)?, "currency": r.get::<_, String>(8)?, "frequency": r.get::<_, String>(9)?, "nextDate": next,
                       "endDate": r.get::<_, Option<String>>(11)?, "linkedGoalId": r.get::<_, Option<String>>(12)?, "active": r.get::<_, i64>(13)? != 0,
                       "due": next.as_str() <= today.as_str() }))
        })?
        .collect::<std::result::Result<_, _>>()?;
    Ok(rows)
}

pub fn save_recurring(conn: &Connection, id: Option<&str>, p: &Value) -> Result<Value> {
    let ttype = p.get("type").and_then(|v| v.as_str()).unwrap_or("expense");
    if !matches!(ttype, "income" | "expense") {
        return invalid("type must be income or expense");
    }
    let freq = p.get("frequency").and_then(|v| v.as_str()).unwrap_or("monthly");
    if !matches!(freq, "weekly" | "monthly" | "yearly") {
        return invalid("frequency must be weekly, monthly or yearly");
    }
    let title = p.get("title").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if title.is_empty() {
        return invalid("give the bill a name");
    }
    let amt = amount(p, "amount")?;
    if amt <= 0 {
        return invalid("amount must be greater than 0");
    }
    let next = p.get("nextDate").and_then(|v| v.as_str()).map(String::from).unwrap_or_else(|| tasks::today(conn));
    if crate::domain::growth::date(&next).is_none() {
        return invalid("date must be YYYY-MM-DD");
    }
    let acc = p.get("accountId").and_then(|v| v.as_str()).ok_or_else(|| AppError::Invalid("choose an account".into()))?;
    account_currency(conn, acc)?;
    let s = |k: &str| p.get(k).and_then(|v| v.as_str()).filter(|v| !v.is_empty()).map(String::from);
    let id = id.map(String::from).unwrap_or_else(new_id);
    conn.execute(
        "INSERT INTO recurring_transactions(id, title, type, account_id, category_id, amount_minor, frequency, next_date, end_date, linked_goal_id, active, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
         ON CONFLICT(id) DO UPDATE SET title=excluded.title, type=excluded.type, account_id=excluded.account_id, category_id=excluded.category_id,
           amount_minor=excluded.amount_minor, frequency=excluded.frequency, next_date=excluded.next_date, end_date=excluded.end_date,
           linked_goal_id=excluded.linked_goal_id, active=excluded.active",
        params![id, title, ttype, acc, s("categoryId"), amt, freq, next, s("endDate"), s("linkedGoalId"), p.get("active").and_then(|v| v.as_bool()).unwrap_or(true) as i64, now()],
    )?;
    audit(conn, "recurring", &id, "save", None, Some(p))?;
    recurring(conn)?.into_iter().find(|r| r["id"] == id.as_str()).ok_or_else(|| AppError::NotFound("recurring".into()))
}

pub fn delete_recurring(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM recurring_transactions WHERE id=?1", [id])?;
    audit(conn, "recurring", id, "delete", None, None)
}

/// Records the due occurrence as a real transaction (post=true) or skips it, then moves to the next date.
pub fn settle_recurring(conn: &Connection, id: &str, post: bool) -> Result<Value> {
    let r = recurring(conn)?.into_iter().find(|r| r["id"] == id).ok_or_else(|| AppError::NotFound("recurring".into()))?;
    let tx = crate::infrastructure::database::begin(conn)?;
    if post {
        add_transaction(&tx, &json!({
            "type": r["type"], "accountId": r["accountId"], "amount": r["amountMinor"], "categoryId": r["categoryId"],
            "occurredOn": r["nextDate"], "merchantOrSource": r["title"], "linkedGoalId": r["linkedGoalId"], "note": "recurring",
        }))?;
    }
    let next = advance(r["nextDate"].as_str().unwrap_or(""), r["frequency"].as_str().unwrap_or("monthly"))?;
    let ended = r["endDate"].as_str().is_some_and(|e| next.as_str() > e);
    tx.execute("UPDATE recurring_transactions SET next_date=?2, active=?3 WHERE id=?1", params![id, next, (!ended) as i64])?;
    tx.commit()?;
    Ok(json!({ "posted": post, "nextDate": next, "ended": ended }))
}

// ---------- CSV import ----------

/// Imports bank-statement style CSV into one account. `mapping` names the columns (date, amount, description,
/// optional category). Negative amounts are expenses, positive are income. Rows already present
/// (same date, amount and description) are skipped, so importing the same file twice adds nothing.
pub fn import_csv(conn: &Connection, account_id: &str, csv: &str, mapping: &Value, dry_run: bool) -> Result<Value> {
    let cur = account_currency(conn, account_id)?;
    let rows = crate::infrastructure::export::parse_csv(csv.trim_start_matches('\u{feff}'));
    let Some(header) = rows.first() else { return invalid("the CSV file is empty") };
    let col = |k: &str| -> Option<usize> {
        let name = mapping.get(k).and_then(|v| v.as_str())?;
        header.iter().position(|h| h.trim() == name)
    };
    let (Some(dc), Some(ac)) = (col("date"), col("amount")) else { return invalid("choose the date and amount columns") };
    let desc_c = col("description");
    let cat_c = col("category");
    let cats = categories(conn)?;
    let parse_date = |s: &str| -> Option<String> {
        let s = s.trim();
        for f in ["%Y-%m-%d", "%d/%m/%Y", "%d-%m-%Y", "%m/%d/%Y", "%Y/%m/%d"] {
            if let Ok(d) = chrono::NaiveDate::parse_from_str(s, f) {
                // Thai Buddhist-era years (e.g. 2569) → Common Era.
                use chrono::Datelike;
                let d = if d.year() > 2400 { d.with_year(d.year() - 543).unwrap_or(d) } else { d };
                return Some(d.to_string());
            }
        }
        None
    };
    let mut preview = vec![];
    let (mut added, mut dupes, mut errors) = (0, 0, vec![]);
    let tx = crate::infrastructure::database::begin(conn)?;
    for (i, r) in rows.iter().enumerate().skip(1) {
        let line = i + 1;
        let Some(date) = r.get(dc).and_then(|s| parse_date(s)) else {
            errors.push(format!("line {line}: unreadable date"));
            continue;
        };
        let raw = r.get(ac).map(|s| s.replace(['(', ')'], "")).unwrap_or_default();
        let negative_paren = r.get(ac).is_some_and(|s| s.trim().starts_with('('));
        let Some(mut minor) = crate::domain::growth::parse_money(&raw) else {
            errors.push(format!("line {line}: unreadable amount “{raw}”"));
            continue;
        };
        if negative_paren {
            minor = -minor.abs();
        }
        if minor == 0 {
            continue;
        }
        let desc = desc_c.and_then(|c| r.get(c)).map(|s| s.trim().to_string()).unwrap_or_default();
        let ttype = if minor < 0 { "expense" } else { "income" };
        let cat = cat_c.and_then(|c| r.get(c)).and_then(|name| cats.iter().find(|x| x["name"].as_str() == Some(name.trim()) && x["kind"] == ttype)).and_then(|x| x["id"].as_str().map(String::from));
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM transactions WHERE deleted_at IS NULL AND account_id=?1 AND occurred_on=?2 AND amount_minor=?3 AND type=?4 AND merchant_or_source=?5)",
            params![account_id, date, minor.abs(), ttype, desc],
            |r| r.get(0),
        )?;
        if exists {
            dupes += 1;
            continue;
        }
        if preview.len() < 20 {
            preview.push(json!({ "date": date, "type": ttype, "amountMinor": minor.abs(), "description": desc, "currency": cur }));
        }
        if !dry_run {
            add_transaction(&tx, &json!({ "type": ttype, "accountId": account_id, "amount": minor.abs(), "occurredOn": date, "merchantOrSource": desc, "categoryId": cat, "note": "imported" }))?;
        }
        added += 1;
    }
    if dry_run {
        drop(tx); // nothing written
    } else {
        tx.commit()?;
    }
    Ok(json!({ "dryRun": dry_run, "rows": rows.len().saturating_sub(1), "toAdd": added, "duplicates": dupes, "errors": errors, "preview": preview, "columns": header }))
}

/// A transaction proposed by an AI client: stored as a draft until the user applies it.
pub fn draft_transaction(conn: &Connection, p: &Value, source: &str) -> Result<Value> {
    let ttype = p.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if !matches!(ttype, "income" | "expense") {
        return invalid("type must be income or expense");
    }
    amount(p, "amount")?;
    let id = new_id();
    conn.execute(
        "INSERT INTO drafts(id, kind, target_id, payload_json, source, created_at) VALUES (?1,'transaction',NULL,?2,?3,?4)",
        params![id, p.to_string(), source, now()],
    )?;
    Ok(json!({ "draftId": id, "status": "pending", "message": "Saved as a draft. The user confirms it in Finance; nothing was recorded yet." }))
}

pub fn transaction_drafts(conn: &Connection) -> Result<Vec<Value>> {
    let rows = conn
        .prepare("SELECT id, payload_json, source, created_at FROM drafts WHERE kind='transaction' AND status='pending' ORDER BY created_at")?
        .query_map([], |r| Ok(json!({ "id": r.get::<_, String>(0)?, "payload": serde_json::from_str::<Value>(&r.get::<_, String>(1)?).unwrap_or(Value::Null), "source": r.get::<_, String>(2)?, "createdAt": r.get::<_, String>(3)? })))?
        .collect::<std::result::Result<_, _>>()?;
    Ok(rows)
}

/// Applies (with the account the user picked) or discards a transaction draft.
pub fn resolve_transaction_draft(conn: &Connection, id: &str, account_id: Option<&str>) -> Result<Value> {
    let payload: String = conn
        .query_row("SELECT payload_json FROM drafts WHERE id=?1 AND kind='transaction' AND status='pending'", [id], |r| r.get(0))
        .optional()?
        .ok_or_else(|| AppError::NotFound("draft".into()))?;
    let tx = crate::infrastructure::database::begin(conn)?;
    let applied = match account_id {
        Some(acc) => {
            let mut p: Value = serde_json::from_str(&payload)?;
            p["accountId"] = json!(acc);
            add_transaction(&tx, &p)?;
            true
        }
        None => false,
    };
    tx.execute("UPDATE drafts SET status=?2, resolved_at=?3 WHERE id=?1", params![id, if applied { "applied" } else { "discarded" }, now()])?;
    tx.commit()?;
    Ok(json!({ "applied": applied }))
}
