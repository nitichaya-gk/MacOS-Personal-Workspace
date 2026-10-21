import { rpc } from "./api";
import type {
  Account, Category, Draft, Evidence, FocusSession, Goal, GoalMetrics, GoalSkill, Habit, HabitSchedule, Milestone,
  MonthSummary, Requirement, SavingsGoal, Skill, Task, Transaction,
} from "../../shared/types";

type Obj = Record<string, unknown>;

export interface GoalDetail {
  goal: Goal;
  requirements: Requirement[];
  milestones: Milestone[];
  tasks: Task[];
  metrics: GoalMetrics;
  drafts: Draft[];
  finance: { spent: { currency: string; spentMinor: number }[]; savingsGoals: SavingsGoal[]; transactions: Transaction[] };
}

export interface GoalOption {
  id: string;
  title: string;
  requirements: { id: string; title: string }[];
  milestones: { id: string; title: string; phase: string; requirementId: string | null }[];
}

export interface ReviewSummary {
  period: string;
  start: string;
  end: string;
  completedTasks: { id: string; title: string }[];
  slippedTasks: { id: string; title: string; dueDate: string }[];
  milestonesMet: { id: string; title: string; goal: string }[];
  evidence: { id: string; title: string; skill: string | null; level: number | null; type: string; observedAt: string }[];
  habits: { id: string; title: string; due: number; done: number; skipped: number; perWeek: number | null; consistency: number | null }[];
  focusMinutes: number;
  goals: { id: string; title: string; status: string; health: string; preparation: number | null; taskCompletion: number | null; focusMinutes: number; plannedMinutes: number | null; preparationDelta?: number }[];
  spending: { currency: string; category: string; totalMinor: number }[];
  previousReview: boolean;
}

export const growth = {
  goals: {
    list: (domain?: string | null, includeClosed = false) => rpc<Goal[]>("goals.list", { domain, includeClosed }),
    detail: (id: string) => rpc<GoalDetail>("goals.detail", { id }),
    options: () => rpc<GoalOption[]>("goals.options"),
    create: (goal: Obj) => rpc<Goal>("goals.create", { goal }),
    update: (id: string, patch: Obj) => rpc<Goal>("goals.update", { id, patch }),
    remove: (id: string) => rpc("goals.delete", { id }),
    restore: (id: string) => rpc<Goal>("goals.restore", { id }),
    deleteForever: (id: string) => rpc("goals.deleteForever", { id }),
    trash: () => rpc<{ id: string; title: string; deletedAt: string }[]>("goals.trash"),
    createExample: () => rpc<Goal>("goals.createExample"),
    addRequirement: (goalId: string, requirement: Obj) => rpc<Requirement>("goals.addRequirement", { goalId, requirement }),
    updateRequirement: (id: string, patch: Obj) => rpc<Requirement>("goals.updateRequirement", { id, patch }),
    deleteRequirement: (id: string) => rpc("goals.deleteRequirement", { id }),
    addMilestone: (goalId: string, milestone: Obj) => rpc<Milestone>("goals.addMilestone", { goalId, milestone }),
    updateMilestone: (id: string, patch: Obj) => rpc<Milestone>("goals.updateMilestone", { id, patch }),
    deleteMilestone: (id: string) => rpc("goals.deleteMilestone", { id }),
    addMilestoneTask: (milestoneId: string, task: Obj) => rpc<Task>("goals.addMilestoneTask", { milestoneId, task }),
    setSkill: (goalId: string, skillId: string, targetLevel: number | null) => rpc("goals.setSkill", { goalId, skillId, targetLevel }),
    drafts: (goalId?: string) => rpc<Draft[]>("goals.drafts", { goalId }),
    resolveDraft: (id: string, apply: boolean) => rpc<{ applied: boolean; created: { requirements: number; milestones: number; tasks: number } }>("goals.resolveDraft", { id, apply }),
  },
  skills: {
    list: () => rpc<Skill[]>("skills.list"),
    get: (id: string) => rpc<Skill>("skills.get", { id }),
    create: (skill: Obj) => rpc<Skill>("skills.create", { skill }),
    update: (id: string, patch: Obj) => rpc<Skill>("skills.update", { id, patch }),
    remove: (id: string) => rpc("skills.delete", { id }),
    evidence: (skillId: string) => rpc<Evidence[]>("skills.evidence", { skillId }),
    addEvidence: (evidence: Obj) => rpc<Evidence>("skills.addEvidence", { evidence }),
    deleteEvidence: (id: string) => rpc("skills.deleteEvidence", { id }),
  },
  focus: {
    running: () => rpc<FocusSession | null>("focus.running"),
    start: (session: Obj) => rpc<FocusSession>("focus.start", { session }),
    stop: (id: string, patch: Obj = {}) => rpc<FocusSession>("focus.stop", { id, patch }),
    discard: (id: string) => rpc("focus.discard", { id }),
  },
  habits: {
    list: (includeArchived = false) => rpc<Habit[]>("habits.list", { includeArchived }),
    create: (habit: Obj & { schedule?: HabitSchedule }) => rpc<Habit>("habits.create", { habit }),
    update: (id: string, patch: Obj) => rpc<Habit>("habits.update", { id, patch }),
    remove: (id: string) => rpc("habits.delete", { id }),
    log: (habitId: string, date: string, status: "done" | "missed" | "skipped" | null, value?: number | null) => rpc<Habit>("habits.log", { habitId, date, status, value }),
  },
  finance: {
    accounts: (includeArchived = false) => rpc<Account[]>("finance.accounts", { includeArchived }),
    createAccount: (account: Obj) => rpc<Account>("finance.createAccount", { account }),
    updateAccount: (id: string, patch: Obj) => rpc("finance.updateAccount", { id, patch }),
    categories: () => rpc<Category[]>("finance.categories"),
    createCategory: (name: string, kind: "income" | "expense") => rpc<Category>("finance.createCategory", { name, kind }),
    addTransaction: (transaction: Obj) => rpc<Transaction>("finance.addTransaction", { transaction }),
    updateTransaction: (id: string, patch: Obj) => rpc<Transaction>("finance.updateTransaction", { id, patch }),
    deleteTransaction: (id: string) => rpc("finance.deleteTransaction", { id }),
    restoreTransaction: (id: string) => rpc("finance.restoreTransaction", { id }),
    transactions: (q: { month?: string; accountId?: string; goalId?: string } = {}) => rpc<Transaction[]>("finance.transactions", q),
    month: (month: string) => rpc<MonthSummary>("finance.month", { month }),
    setBudget: (categoryId: string, limit: string | null, currency = "THB") => rpc("finance.setBudget", { categoryId, limit, currency }),
    savingsGoals: () => rpc<SavingsGoal[]>("finance.savingsGoals"),
    createSavingsGoal: (savingsGoal: Obj) => rpc<{ id: string }>("finance.createSavingsGoal", { savingsGoal }),
    deleteSavingsGoal: (id: string) => rpc("finance.deleteSavingsGoal", { id }),
    allocate: (id: string, amount: string, note = "") => rpc("finance.allocate", { id, amount, note }),
    audit: () => rpc<{ entityType: string; entityId: string; action: string; at: string }[]>("finance.audit"),
  },
  today: () =>
    rpc<{ date: string; tasks: Task[]; habits: Habit[]; goals: Goal[]; runningSession: FocusSession | null; focusMinutesToday: number }>("growth.today"),
  reviews: {
    currentWeek: () => rpc<{ period: string }>("reviews.currentWeek"),
    get: (period?: string) => rpc<{ period: string; draft: ReviewSummary; saved: { reflection: string; learnings: string; nextCommitments: string; updatedAt: string } | null }>("reviews.get", { period }),
    save: (period: string, review: Obj) => rpc<{ period: string; draft: ReviewSummary; saved: { reflection: string; learnings: string; nextCommitments: string; updatedAt: string } | null }>("reviews.save", { period, review }),
    list: () => rpc<{ period: string; updatedAt: string }[]>("reviews.list"),
  },
};

export type { GoalSkill };

/** Minor units → "1,234.50" (or a mask when the privacy shield is on). */
export function money(minor: number, currency: string, locale: string, hidden = false): string {
  if (hidden) return "••••";
  return new Intl.NumberFormat(locale, { style: "currency", currency, minimumFractionDigits: 2 }).format(minor / 100);
}

export const pct = (v: number | null | undefined) => (v === null || v === undefined ? "—" : `${Math.round(v * 100)}%`);
