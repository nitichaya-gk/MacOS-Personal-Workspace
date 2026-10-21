// Shapes returned by the growth services (goals, skills, habits, finance, reviews).

export type GoalDomain = "education" | "career" | "finance" | "personal" | "project";
export type GoalStatus = "draft" | "active" | "paused" | "achieved" | "cancelled";
export type HealthState = "on_track" | "at_risk" | "blocked" | "needs_review" | "inactive" | "achieved";

export interface Goal {
  id: string;
  title: string;
  description: string;
  domain: GoalDomain;
  motivation: string;
  startDate: string | null;
  targetDate: string | null;
  status: GoalStatus;
  priority: number;
  timeBudgetHoursPerWeek: number | null;
  budgetLimitMinor: number | null;
  currency: string;
  successDefinition: string;
  riskLevel: "low" | "medium" | "high";
  confidence: string;
  notes: string;
  projectId: string | null;
  lastReviewedAt: string | null;
  isExample: boolean;
  createdAt: string;
  updatedAt: string;
  version: number;
  metrics?: GoalListMetrics;
}

export interface Health {
  state: HealthState;
  reasons: string[];
}

export interface GoalListMetrics {
  taskCompletion: number | null;
  roadmapProgress: number | null;
  preparation: number | null;
  unknownCount: number;
  unverified: number;
  skillReadiness: number | null;
  health: Health;
  tasksDone: number;
  tasksTotal: number;
  requirementsMet: number;
  requirementsTotal: number;
}

export type RequirementType = "binary" | "numeric" | "score" | "document" | "portfolio" | "external_verification";
export type Satisfaction = "not_started" | "in_progress" | "met" | "not_met" | "blocked" | "unknown";

export interface Evidence {
  id: string;
  skillId: string;
  evidenceType: string;
  title: string;
  level: number | null;
  score: number | null;
  maxScore: number | null;
  rubricVersion: number | null;
  taskId: string | null;
  requirementId: string | null;
  sourceItemId: string | null;
  sourceUrl: string;
  observedAt: string;
  reviewer: string;
  notes: string;
  skillName: string | null;
  taskTitle: string | null;
  sourceTitle: string | null;
}

export interface Requirement {
  id: string;
  goalId: string;
  title: string;
  description: string;
  type: RequirementType;
  critical: boolean;
  weight: number;
  targetValue: number | null;
  currentValue: number | null;
  unit: string;
  deadline: string | null;
  verificationSource: string;
  sourceCheckedAt: string | null;
  verificationStatus: "unverified" | "verified" | "outdated";
  satisfactionStatus: Satisfaction;
  manualOverrideReason: string;
  evidence: Evidence[];
  taskCount: number;
}

export interface Milestone {
  id: string;
  goalId: string;
  requirementId: string | null;
  phase: string;
  title: string;
  dueDate: string | null;
  status: "planned" | "in_progress" | "met" | "missed";
  acceptanceCriteria: string;
  weight: number;
  position: number;
}

export interface ReqProgress {
  id: string;
  progress: number | null;
  explanation: string;
  needsVerification: boolean;
  verificationState: string;
}

export interface GoalSkill {
  skillId: string;
  name: string;
  targetLevel: number;
  currentLevel: number | null;
  confidence: string;
  lastAssessedAt: string | null;
  stale: boolean;
  evidenceCount: number;
  gap: number | null;
}

export interface GoalMetrics {
  taskCompletion: { value: number | null; done: number; total: number; formula: string; note: string };
  roadmapProgress: { value: number | null; milestones: number; met: number; formula: string };
  preparation: { value: number | null; met: number; total: number; unknownCount: number; unknownWeight: number; knownWeight: number; requirements: ReqProgress[]; unverified: number; formula: string };
  skillReadiness: { value: number | null; assessed: number; total: number; skills: GoalSkill[]; formula: string };
  health: Health;
  achievementBlockers: string[];
  overdueTasks: number;
}

export interface Draft {
  id: string;
  kind: string;
  targetId: string | null;
  payload: { rationale?: string; requirements: { title: string; type?: string; milestones?: { title: string; phase?: string; tasks?: { title: string; estimatedMinutes?: number }[] }[] }[] };
  source: string;
  createdAt: string;
}

export interface Skill {
  id: string;
  name: string;
  category: string;
  description: string;
  targetLevel: number;
  rubric: string[];
  rubricVersion: number;
  readiness: { currentLevel: number | null; lastAssessedAt: string | null; confidence: string; evidenceCount: number; recentEvidenceCount: number; stale: boolean };
  practiceTasks: number;
  goals: { id: string; title: string; targetLevel: number }[];
}

export interface FocusSession {
  id: string;
  taskId: string | null;
  goalId: string | null;
  skillId: string | null;
  startedAt: string;
  endedAt: string | null;
  plannedMinutes: number | null;
  actualMinutes: number | null;
  note: string;
  taskTitle: string | null;
  goalTitle: string | null;
}

export type HabitSchedule = { type: "daily" } | { type: "weekdays"; days: number[] } | { type: "weekly"; perWeek: number };

export interface Habit {
  id: string;
  title: string;
  description: string;
  kind: "binary" | "count" | "duration" | "numeric";
  targetValue: number | null;
  unit: string;
  schedule: HabitSchedule;
  goalId: string | null;
  skillId: string | null;
  goalTitle: string | null;
  skillName: string | null;
  reminderTime: string | null;
  startDate: string;
  archivedAt: string | null;
  stats: { streak: number; streakUnit: string; done7: number; due7: number; skipped7: number; consistency7: number | null; weekDone: number; weekTarget: number | null; dueToday: boolean; todayStatus: string | null };
  recent: { date: string; status: "done" | "missed" | "skipped" | "not_due" | "open"; logged: boolean }[];
}

export interface Account {
  id: string;
  name: string;
  type: string;
  currency: string;
  openingBalanceMinor: number;
  balanceMinor: number;
  archivedAt: string | null;
}

export interface Category {
  id: string;
  name: string;
  kind: "income" | "expense";
  color: string;
}

export interface Transaction {
  id: string;
  accountId: string;
  type: "income" | "expense" | "transfer";
  amountMinor: number;
  currency: string;
  categoryId: string | null;
  occurredOn: string;
  merchantOrSource: string;
  note: string;
  linkedGoalId: string | null;
  transferGroupId: string | null;
  accountName: string;
  categoryName: string | null;
  goalTitle: string | null;
}

export interface MonthSummary {
  month: string;
  currencies: {
    currency: string;
    incomeMinor: number;
    expenseMinor: number;
    netFlowMinor: number;
    byCategory: { categoryId: string | null; name: string | null; kind: string | null; color: string | null; totalMinor: number }[];
    budgets: { categoryId: string; name: string; limitMinor: number; spentMinor: number; remainingMinor: number; over: boolean; goalId: string | null }[];
    totalBalanceMinor: number;
    allocatedSavingsMinor: number;
    availableCashMinor: number;
  }[];
}

export interface SavingsGoal {
  id: string;
  title: string;
  targetMinor: number;
  currency: string;
  targetDate: string | null;
  goalId: string | null;
  goalTitle: string | null;
  savedMinor: number;
  progress: number;
}
