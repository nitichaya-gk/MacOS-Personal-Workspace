import { useCallback, useEffect, useState } from "react";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { growth, money } from "../../services/desktop-bridge/growthApi";
import type { Account, Category, Goal, MonthSummary, SavingsGoal, Transaction } from "../../shared/types";
import { Icon } from "../../components/ui/Icon";
import { EmptyState, Modal } from "../../components/ui/Modal";
import { Bar } from "../goals/ui";

/** Amounts are hidden until revealed (privacy shield). Per-viewer preference only. */
export function useShield(): [boolean, () => void] {
  const read = () => {
    try {
      return localStorage.getItem("financeShield") !== "off";
    } catch {
      return true;
    }
  };
  const [hidden, setHidden] = useState(read);
  useEffect(() => {
    const on = () => setHidden(read());
    window.addEventListener("pw:shield", on);
    return () => window.removeEventListener("pw:shield", on);
  }, []);
  const toggle = () => {
    try {
      localStorage.setItem("financeShield", hidden ? "off" : "on");
    } catch {
      /* convenience only */
    }
    setHidden(!hidden);
    window.dispatchEvent(new Event("pw:shield"));
  };
  return [hidden, toggle];
}

const thisMonth = (today?: string) => (today ?? new Date().toISOString()).slice(0, 7);
const shiftMonth = (m: string, n: number) => {
  const [y, mo] = m.split("-").map(Number);
  const d = new Date(Date.UTC(y, mo - 1 + n, 1));
  return d.toISOString().slice(0, 7);
};

export function FinanceView() {
  const s = useStore();
  const { t, locale } = useI18n();
  const [hidden, toggleShield] = useShield();
  const [month, setMonth] = useState(thisMonth(s.counts?.date));
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [categories, setCategories] = useState<Category[]>([]);
  const [summary, setSummary] = useState<MonthSummary | null>(null);
  const [txs, setTxs] = useState<Transaction[]>([]);
  const [savings, setSavings] = useState<SavingsGoal[]>([]);
  const [goals, setGoals] = useState<Goal[]>([]);
  const [adding, setAdding] = useState<null | "income" | "expense" | "transfer">(null);
  const [newAccount, setNewAccount] = useState(false);
  const [newSaving, setNewSaving] = useState(false);

  const load = useCallback(() => {
    growth.finance.accounts().then(setAccounts).catch(s.fail);
    growth.finance.categories().then(setCategories).catch(s.fail);
    growth.finance.month(month).then(setSummary).catch(s.fail);
    growth.finance.transactions({ month }).then(setTxs).catch(s.fail);
    growth.finance.savingsGoals().then(setSavings).catch(s.fail);
    growth.goals.list().then(setGoals).catch(() => {});
  }, [month, s.fail]);
  useEffect(load, [load]);

  const m = (minor: number, cur: string) => money(minor, cur, locale, hidden);
  const label = new Date(`${month}-01T00:00:00Z`).toLocaleDateString(locale, { month: "long", year: "numeric", timeZone: "UTC" });

  return (
    <div className="content">
      <div className="page" style={{ maxWidth: 1040 }}>
        <div className="row" style={{ marginTop: 16 }}>
          <h1 style={{ margin: 0, fontSize: 30 }}>💰 {t("Personal finance")}</h1>
          <span className="spacer" />
          <button className="btn" onClick={toggleShield} aria-pressed={!hidden}>{hidden ? `👁 ${t("Show amounts")}` : `🙈 ${t("Hide amounts")}`}</button>
        </div>
        <p className="muted small-text">{t("Manual tracking only — no bank connection, nothing leaves this Mac. Not financial advice. AI clients cannot read this unless you share finance in Settings → MCP.")}</p>

        {accounts.length === 0 ? (
          <EmptyState icon="👛" title={t("Add an account to start")} hint={t("e.g. Cash, Savings account, e-wallet. Enter the current balance as the opening balance.")}>
            <button className="btn primary" onClick={() => setNewAccount(true)}>＋ {t("Add account")}</button>
          </EmptyState>
        ) : (
          <>
            <div className="row" style={{ margin: "8px 0 12px" }}>
              <button className="icon-btn" aria-label={t("Previous month")} onClick={() => setMonth(shiftMonth(month, -1))}><Icon name="chevronLeft" /></button>
              <strong style={{ fontSize: 16, minWidth: 150, textAlign: "center" }}>{label}</strong>
              <button className="icon-btn" aria-label={t("Next month")} onClick={() => setMonth(shiftMonth(month, 1))}><Icon name="chevronRight" /></button>
              <span className="spacer" />
              <button className="btn small primary" onClick={() => setAdding("expense")}>－ {t("Expense")}</button>
              <button className="btn small" onClick={() => setAdding("income")}>＋ {t("Income")}</button>
              <button className="btn small" onClick={() => setAdding("transfer")}>⇄ {t("Transfer")}</button>
            </div>

            {summary?.currencies.map((c) => (
              <div key={c.currency}>
                <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(170px, 1fr))", gap: 10 }}>
                  {[
                    [t("Income"), c.incomeMinor, "var(--success)"],
                    [t("Expenses"), c.expenseMinor, "var(--danger)"],
                    [t("Net flow"), c.netFlowMinor, c.netFlowMinor >= 0 ? "var(--success)" : "var(--danger)"],
                    [t("Available cash"), c.availableCashMinor, "var(--text)"],
                    [t("Allocated to savings"), c.allocatedSavingsMinor, "var(--accent)"],
                  ].map(([k, v, color]) => (
                    <div key={k as string} className="card-box" style={{ marginBottom: 0, padding: "10px 14px" }}>
                      <div className="small-text muted">{k as string} · {c.currency}</div>
                      <div style={{ fontSize: 20, fontWeight: 700, color: color as string }}>{m(v as number, c.currency)}</div>
                    </div>
                  ))}
                </div>
                <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16, marginTop: 16 }}>
                  <div>
                    <div className="h-section" style={{ marginTop: 0 }}>{t("Budgets this month")}</div>
                    {categories.filter((x) => x.kind === "expense").map((cat) => {
                      const b = c.budgets.find((x) => x.categoryId === cat.id);
                      const spent = c.byCategory.find((x) => x.categoryId === cat.id)?.totalMinor ?? 0;
                      return (
                        <div key={cat.id} style={{ marginBottom: 8 }}>
                          <div className="row small-text">
                            <span style={{ flex: 1 }}>{t(cat.name)}</span>
                            <span className={b?.over ? "" : "muted"} style={b?.over ? { color: "var(--danger)", fontWeight: 600 } : undefined}>
                              {b?.over && "⚠︎ "}{m(spent, c.currency)}{b ? ` / ${m(b.limitMinor, c.currency)}` : ""}
                            </span>
                            <button className="btn small ghost" onClick={async () => {
                              const v = await s.prompt({ title: t("Monthly budget for {name}", { name: t(cat.name) }), defaultValue: b ? (b.limitMinor / 100).toFixed(2) : "", placeholder: t("Leave empty to remove") });
                              if (v === null) return;
                              growth.finance.setBudget(cat.id, v.trim() ? v : null, c.currency).then(load).catch(s.fail);
                            }}>{b ? t("Edit") : t("Set budget")}</button>
                          </div>
                          {b && <Bar value={b.spentMinor / b.limitMinor} color={b.over ? "var(--danger)" : "var(--accent)"} label={cat.name} />}
                        </div>
                      );
                    })}
                  </div>
                  <div>
                    <div className="h-section" style={{ marginTop: 0 }}>{t("Where money went")}</div>
                    {c.byCategory.filter((x) => x.kind === "expense").length === 0 && <div className="muted small-text">{t("No expenses this month.")}</div>}
                    {c.byCategory.filter((x) => x.kind === "expense").map((x) => (
                      <div key={x.categoryId ?? "none"} className="row small-text" style={{ marginBottom: 6 }}>
                        <span style={{ width: 130 }}>{t(x.name ?? "Uncategorised")}</span>
                        <div style={{ flex: 1 }}><Bar value={c.expenseMinor ? x.totalMinor / c.expenseMinor : 0} color={`var(--opt-${x.color ?? "gray"})`} /></div>
                        <span style={{ width: 90, textAlign: "right" }}>{m(x.totalMinor, c.currency)}</span>
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            ))}

            <div className="h-section">
              <span>{t("Savings goals")}</span><span className="spacer" />
              <button className="btn small ghost" onClick={() => setNewSaving(true)}>＋ {t("Savings goal")}</button>
            </div>
            <p className="small-text faint" style={{ marginTop: -4 }}>{t("Allocating earmarks money you already have; it does not create money. Available cash = balances − allocations.")}</p>
            {savings.map((x) => (
              <div key={x.id} className="row" style={{ marginBottom: 8 }}>
                <span style={{ width: 220 }}>{x.title}{x.goalTitle && <span className="small-text faint"> · 🎯 {x.goalTitle}</span>}</span>
                <div style={{ flex: 1 }}><Bar value={x.progress} color="var(--success)" label={x.title} /></div>
                <span className="small-text" style={{ width: 200, textAlign: "right" }}>{m(x.savedMinor, x.currency)} / {m(x.targetMinor, x.currency)}{x.targetDate && ` · ${x.targetDate}`}</span>
                <button className="btn small" onClick={async () => {
                  const v = await s.prompt({ title: t("Allocate to “{name}”", { name: x.title }), placeholder: t("Amount (negative to release)") });
                  if (v?.trim()) growth.finance.allocate(x.id, v).then(load).catch(s.fail);
                }}>{t("Allocate")}</button>
                <button className="icon-btn small" aria-label={t("Delete")} onClick={async () => { if (await s.confirm({ title: t("Delete this savings goal?"), danger: true, confirmLabel: t("Delete") })) growth.finance.deleteSavingsGoal(x.id).then(load).catch(s.fail); }}><Icon name="trash" size={13} /></button>
              </div>
            ))}

            <div className="h-section"><span>{t("Accounts")}</span><span className="spacer" /><button className="btn small ghost" onClick={() => setNewAccount(true)}>＋ {t("Account")}</button></div>
            <div className="home-grid">
              {accounts.map((a) => (
                <div key={a.id} className="tile" style={{ minHeight: 0 }}>
                  <span className="small-text muted">{t(a.type)} · {a.currency}</span>
                  <span className="tile-title">{a.name}</span>
                  <span style={{ fontWeight: 600 }}>{m(a.balanceMinor, a.currency)}</span>
                </div>
              ))}
            </div>

            <div className="h-section">{t("Transactions")} · {label}</div>
            {txs.length === 0 ? <div className="muted small-text">{t("No transactions this month.")}</div> : (
              <table className="plain">
                <thead><tr><th>{t("Date")}</th><th>{t("Description")}</th><th>{t("Category")}</th><th>{t("Account")}</th><th style={{ textAlign: "right" }}>{t("Amount")}</th><th /></tr></thead>
                <tbody>
                  {txs.map((x) => (
                    <tr key={x.id}>
                      <td className="muted">{x.occurredOn}</td>
                      <td>{x.type === "transfer" ? `⇄ ${t("Transfer")}` : x.merchantOrSource || "—"}{x.note && <span className="faint"> · {x.note}</span>}{x.goalTitle && <span className="chip" style={{ marginLeft: 6 }}>🎯 {x.goalTitle}</span>}</td>
                      <td>{x.categoryName ? t(x.categoryName) : "—"}</td>
                      <td className="muted">{x.accountName}</td>
                      <td style={{ textAlign: "right", color: x.type === "income" ? "var(--success)" : x.type === "expense" ? "var(--danger)" : undefined }}>
                        {x.type === "expense" ? "−" : x.type === "income" ? "+" : x.amountMinor < 0 ? "−" : "+"}{m(Math.abs(x.amountMinor), x.currency)}
                      </td>
                      <td>
                        <button className="icon-btn small" aria-label={t("Delete")} onClick={() =>
                          growth.finance.deleteTransaction(x.id).then(() => {
                            load();
                            s.toast(t("Transaction deleted"), { action: { label: t("Undo"), run: () => growth.finance.restoreTransaction(x.id).then(load).catch(s.fail) } });
                          }).catch(s.fail)}><Icon name="trash" size={13} /></button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </>
        )}
      </div>
      {adding && <TransactionDialog type={adding} accounts={accounts} categories={categories} goals={goals} onClose={() => setAdding(null)} onSaved={() => (setAdding(null), load())} />}
      {newAccount && <AccountDialog onClose={() => setNewAccount(false)} onSaved={() => (setNewAccount(false), load())} />}
      {newSaving && <SavingsDialog goals={goals} onClose={() => setNewSaving(false)} onSaved={() => (setNewSaving(false), load())} />}
    </div>
  );
}

function TransactionDialog({ type, accounts, categories, goals, onClose, onSaved }: { type: "income" | "expense" | "transfer"; accounts: Account[]; categories: Category[]; goals: Goal[]; onClose: () => void; onSaved: () => void }) {
  const s = useStore();
  const { t } = useI18n();
  const [kind, setKind] = useState(type);
  const cats = categories.filter((c) => c.kind === kind);
  const [f, setF] = useState({ amount: "", accountId: accounts[0]?.id ?? "", toAccountId: accounts[1]?.id ?? "", categoryId: "", occurredOn: s.counts?.date ?? "", merchantOrSource: "", note: "", linkedGoalId: "" });
  const set = (k: keyof typeof f, v: string) => setF((x) => ({ ...x, [k]: v }));
  const submit = () =>
    growth.finance
      .addTransaction(kind === "transfer"
        ? { type: "transfer", fromAccountId: f.accountId, toAccountId: f.toAccountId, amount: f.amount, occurredOn: f.occurredOn, note: f.note }
        : { type: kind, accountId: f.accountId, amount: f.amount, categoryId: f.categoryId || cats[0]?.id, occurredOn: f.occurredOn, merchantOrSource: f.merchantOrSource, note: f.note, linkedGoalId: f.linkedGoalId || undefined })
      .then(onSaved)
      .catch(s.fail);
  return (
    <Modal title={t("New transaction")} onClose={onClose} footer={<><button className="btn" onClick={onClose}>{t("Cancel")}</button><button className="btn primary" onClick={submit}>{t("Save")}</button></>}>
      <div className="tabs">
        {(["expense", "income", "transfer"] as const).map((k) => <button key={k} className={`tab${kind === k ? " active" : ""}`} onClick={() => setKind(k)}>{t(k[0].toUpperCase() + k.slice(1))}</button>)}
      </div>
      <div className="field"><label>{t("Amount")}</label><input className="input" autoFocus inputMode="decimal" placeholder="0.00" value={f.amount} onChange={(e) => set("amount", e.target.value)} style={{ fontSize: 18, height: 38 }} /></div>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "0 12px" }}>
        <div className="field"><label>{kind === "transfer" ? t("From account") : t("Account")}</label>
          <select className="select" value={f.accountId} onChange={(e) => set("accountId", e.target.value)}>{accounts.map((a) => <option key={a.id} value={a.id}>{a.name} ({a.currency})</option>)}</select>
        </div>
        {kind === "transfer" ? (
          <div className="field"><label>{t("To account")}</label>
            <select className="select" value={f.toAccountId} onChange={(e) => set("toAccountId", e.target.value)}>{accounts.map((a) => <option key={a.id} value={a.id}>{a.name} ({a.currency})</option>)}</select>
          </div>
        ) : (
          <div className="field"><label>{t("Category")}</label>
            <select className="select" value={f.categoryId || cats[0]?.id} onChange={(e) => set("categoryId", e.target.value)}>{cats.map((c) => <option key={c.id} value={c.id}>{t(c.name)}</option>)}</select>
          </div>
        )}
        <div className="field"><label>{t("Date")}</label><input className="input" type="date" value={f.occurredOn} onChange={(e) => set("occurredOn", e.target.value)} /></div>
        {kind !== "transfer" && <div className="field"><label>{kind === "income" ? t("Source") : t("Paid to")}</label><input className="input" value={f.merchantOrSource} onChange={(e) => set("merchantOrSource", e.target.value)} /></div>}
      </div>
      {kind !== "transfer" && (
        <div className="field"><label>{t("For goal (optional)")}</label>
          <select className="select" value={f.linkedGoalId} onChange={(e) => set("linkedGoalId", e.target.value)}>
            <option value="">—</option>{goals.map((g) => <option key={g.id} value={g.id}>{g.title}</option>)}
          </select>
        </div>
      )}
      <div className="field"><label>{t("Note")}</label><input className="input" value={f.note} onChange={(e) => set("note", e.target.value)} /></div>
      {kind === "transfer" && <p className="small-text muted">{t("Transfers move money between your own accounts and never count as income or expense.")}</p>}
    </Modal>
  );
}

function AccountDialog({ onClose, onSaved }: { onClose: () => void; onSaved: () => void }) {
  const s = useStore();
  const { t } = useI18n();
  const [f, setF] = useState({ name: "", type: "cash", currency: "THB", openingBalance: "0" });
  return (
    <Modal title={t("New account")} onClose={onClose} footer={<><button className="btn" onClick={onClose}>{t("Cancel")}</button><button className="btn primary" onClick={() => growth.finance.createAccount(f).then(onSaved).catch(s.fail)}>{t("Create")}</button></>}>
      <div className="field"><label>{t("Name")}</label><input className="input" autoFocus value={f.name} onChange={(e) => setF({ ...f, name: e.target.value })} /></div>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: "0 12px" }}>
        <div className="field"><label>{t("Type")}</label>
          <select className="select" value={f.type} onChange={(e) => setF({ ...f, type: e.target.value })}>{["cash", "bank", "savings", "credit", "ewallet", "other"].map((x) => <option key={x} value={x}>{t(x)}</option>)}</select>
        </div>
        <div className="field"><label>{t("Currency")}</label><input className="input" maxLength={3} value={f.currency} onChange={(e) => setF({ ...f, currency: e.target.value.toUpperCase() })} /></div>
        <div className="field"><label>{t("Current balance")}</label><input className="input" inputMode="decimal" value={f.openingBalance} onChange={(e) => setF({ ...f, openingBalance: e.target.value })} /></div>
      </div>
    </Modal>
  );
}

function SavingsDialog({ goals, onClose, onSaved }: { goals: Goal[]; onClose: () => void; onSaved: () => void }) {
  const s = useStore();
  const { t } = useI18n();
  const [f, setF] = useState({ title: "", target: "", currency: "THB", targetDate: "", goalId: "" });
  return (
    <Modal title={t("New savings goal")} onClose={onClose} footer={<><button className="btn" onClick={onClose}>{t("Cancel")}</button><button className="btn primary" onClick={() => growth.finance.createSavingsGoal(f).then(onSaved).catch(s.fail)}>{t("Create")}</button></>}>
      <div className="field"><label>{t("Name")}</label><input className="input" autoFocus value={f.title} placeholder={t("e.g. Laptop for study")} onChange={(e) => setF({ ...f, title: e.target.value })} /></div>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "0 12px" }}>
        <div className="field"><label>{t("Target amount")}</label><input className="input" inputMode="decimal" value={f.target} onChange={(e) => setF({ ...f, target: e.target.value })} /></div>
        <div className="field"><label>{t("Target date")}</label><input className="input" type="date" value={f.targetDate} onChange={(e) => setF({ ...f, targetDate: e.target.value })} /></div>
      </div>
      <div className="field"><label>{t("For goal (optional)")}</label>
        <select className="select" value={f.goalId} onChange={(e) => setF({ ...f, goalId: e.target.value })}><option value="">—</option>{goals.map((g) => <option key={g.id} value={g.id}>{g.title}</option>)}</select>
      </div>
    </Modal>
  );
}
