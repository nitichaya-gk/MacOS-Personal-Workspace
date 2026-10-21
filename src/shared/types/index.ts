// Mirrors the serde (camelCase) shapes returned by the Rust application layer.

export type PageKind = "page" | "project" | "database" | "record" | "canvas";
export type CanvasKind = "diagram" | "wireframe";

export interface PageMeta {
  id: string;
  sectionId: string | null;
  parentId: string | null;
  kind: PageKind;
  title: string;
  icon: string | null;
  position: number;
  isTemplate: boolean;
  favorite: boolean;
  archivedAt: string | null;
  deletedAt: string | null;
  createdAt: string;
  updatedAt: string;
  lastOpenedAt: string | null;
  canvasKind: CanvasKind | null;
}

export interface Section {
  id: string;
  name: string;
  icon: string | null;
  kind: "projects" | "knowledge" | "databases" | "templates" | "custom";
  position: number;
}

export interface PageDetail {
  meta: PageMeta;
  blocks: unknown[];
  breadcrumb: PageMeta[];
  projectId: string | null;
  sectionName: string | null;
}

export interface Backlink {
  sourceId: string;
  sourceType: "page" | "task" | "canvas";
  title: string;
  icon: string | null;
  kind: PageKind | null;
  context: string;
}

export type PropertyType =
  | "title" | "text" | "number" | "select" | "multi_select" | "status" | "date" | "checkbox"
  | "url" | "email" | "files" | "created_time" | "last_edited_time";

export interface SelectOption {
  id: string;
  name: string;
  color: string;
  group?: "todo" | "in_progress" | "done";
}

export interface Property {
  id: string;
  name: string;
  type: PropertyType;
  config: { options?: SelectOption[]; [k: string]: unknown };
  position: number;
}

export type ViewType = "table" | "board" | "calendar" | "list";

export interface FilterCondition {
  propertyId: string;
  operator: string;
  value?: unknown;
}
export interface FilterGroup {
  op: "and" | "or";
  conditions: (FilterCondition | FilterGroup)[];
}
export interface SortSpec {
  propertyId: string;
  direction: "asc" | "desc";
}

export interface ViewConfig {
  filter?: FilterGroup;
  sorts?: SortSpec[];
  /** Property ids hidden in this view. */
  hidden?: string[];
  groupBy?: string | null;
  dateProperty?: string | null;
  widths?: Record<string, number>;
}

export interface View {
  id: string;
  name: string;
  type: ViewType;
  config: ViewConfig;
  position: number;
}

export interface DatabaseInfo {
  id: string;
  title: string;
  icon: string | null;
  properties: Property[];
  views: View[];
  templates: PageMeta[];
  mcpAccess: "inherit" | "deny";
}

export interface DbRecord {
  id: string;
  title: string;
  icon: string | null;
  createdAt: string;
  updatedAt: string;
  values: Record<string, unknown>;
}

export interface QueryResult {
  total: number;
  offset: number;
  records: DbRecord[];
}

export type TaskStatus = "todo" | "in_progress" | "done";
export type TaskPriority = "none" | "low" | "medium" | "high" | "urgent";

export interface Task {
  id: string;
  title: string;
  description: string;
  status: TaskStatus;
  priority: TaskPriority;
  dueDate: string | null;
  remindAt: string | null;
  remindAtLocal: string | null;
  projectId: string | null;
  projectTitle: string | null;
  sourcePageId: string | null;
  sourcePageTitle: string | null;
  completedAt: string | null;
  deletedAt: string | null;
  createdAt: string;
  updatedAt: string;
  links: { id: string; title: string; kind: PageKind; icon: string | null }[];
  milestoneId: string | null;
  estimatedEffortMinutes: number | null;
  actualEffortMinutes: number | null;
  requirementLinks: { requirementId: string; requirementTitle: string; goalId: string; goalTitle: string; allocationPct: number }[];
  skills: { id: string; name: string }[];
  dependsOn: { id: string; title: string; status: TaskStatus }[];
}

export interface TaskCounts {
  today: number;
  overdue: number;
  upcoming: number;
  date: string;
  timezone: string;
}

export interface SearchHit {
  entityId: string;
  entityType: PageKind | "task";
  title: string;
  snippet: string;
  projectId: string | null;
  updatedAt: string;
  icon: string | null;
}

export interface Settings {
  language: "system" | "en" | "th";
  timezone: string;
  effectiveTimezone: string;
  dailySummary: { enabled: boolean; time: string };
  backup: { auto: boolean; keep: number };
  mcp: { enabled: boolean; scope: "all" | "selected"; allowedIds: string[] };
  privacyShield?: boolean;
  notificationsAsked: boolean;
}

export interface WorkspaceInfo {
  id: string;
  name: string;
  root: string;
  schemaVersion: number;
  pages: number;
  tasks: number;
  assetsBytes: number;
  recentWorkspaces: string[];
  defaultRoot: string;
  appSupportDir: string;
  logsDir: string;
}

export interface AssetInfo {
  id: string;
  fileName: string;
  mime: string;
  size: number;
  pageId: string | null;
  createdAt: string;
  url: string;
}

export interface BackupInfo {
  name: string;
  path: string;
  createdAt: string | null;
  schemaVersion: number | null;
  sizeBytes: number;
}

export type Route =
  | { name: "home" }
  | { name: "page"; id: string; tab?: string }
  | { name: "tasks"; view?: string; taskId?: string }
  | { name: "search"; q?: string }
  | { name: "trash" }
  | { name: "archive" }
  | { name: "templates" }
  | { name: "settings"; tab?: string }
  | { name: "section"; id: string }
  | { name: "goals"; domain?: string }
  | { name: "goal"; id: string; tab?: string }
  | { name: "skills"; id?: string }
  | { name: "habits" }
  | { name: "finance"; tab?: string }
  | { name: "review"; period?: string };

export * from "./growth";
