import { useCallback, useEffect, useState } from "react";
import { useStore } from "../../app/providers/store";
import { useI18n } from "../../app/providers/i18n";
import { growth } from "../../services/desktop-bridge/growthApi";
import { api } from "../../services/desktop-bridge/api";
import type { Course, Goal, Resource, Skill, Topic } from "../../shared/types";
import { Icon } from "../../components/ui/Icon";
import { EmptyState, Modal } from "../../components/ui/Modal";
import { Popover, useAnchor } from "../../components/ui/Popover";

type Overview = Awaited<ReturnType<typeof growth.learning.overview>>;

const TOPIC_STATUS: Record<Topic["status"], { icon: string; label: string; chip: string }> = {
  not_started: { icon: "○", label: "Not started", chip: "c-gray" },
  learning: { icon: "◐", label: "Learning", chip: "c-blue" },
  reviewing: { icon: "↻", label: "Reviewing", chip: "c-yellow" },
  mastered: { icon: "★", label: "Mastered", chip: "c-green" },
};
const COURSE_STATUS = ["planned", "active", "paused", "done"] as const;
const RESOURCE_ICON: Record<Resource["kind"], string> = { link: "🔗", book: "📕", video: "🎬", exercise: "✏️", note: "📝", file: "📎" };

/** Spaced-review answer buttons: quality on the SM-2 0–5 scale. */
const QUALITY = [
  { q: 1, label: "Forgot", hint: "Review again tomorrow" },
  { q: 3, label: "Hard", hint: "Recalled with effort" },
  { q: 4, label: "Good", hint: "Recalled after a moment" },
  { q: 5, label: "Easy", hint: "Instant recall" },
];

export function LearningView() {
  const s = useStore();
  const { t } = useI18n();
  const [data, setData] = useState<Overview | null>(null);
  const [skills, setSkills] = useState<Skill[]>([]);
  const [goals, setGoals] = useState<Goal[]>([]);
  const [editCourse, setEditCourse] = useState<Course | "new" | null>(null);
  const [resourceFor, setResourceFor] = useState<{ courseId?: string; topicId?: string } | null>(null);
  const load = useCallback(() => growth.learning.overview().then(setData).catch(s.fail), [s.fail]);
  useEffect(() => {
    load();
    growth.skills.list().then(setSkills).catch(() => {});
    growth.goals.list().then(setGoals).catch(() => {});
  }, [load]);

  const addTopic = async (courseId: string | null) => {
    const title = await s.prompt({ title: t("New topic"), placeholder: t("e.g. Dynamic programming, Past simple") });
    if (title?.trim()) growth.learning.saveTopic(null, { title, courseId }).then(load).catch(s.fail);
  };

  if (!data) return <div className="content"><div className="loading">…</div></div>;
  const byId = new Map(data.topics.map((x) => [x.id, x]));
  const groups: { course: Course | null; topics: Topic[] }[] = [
    ...data.courses.map((c) => ({ course: c, topics: data.topics.filter((x) => x.courseId === c.id) })),
    { course: null, topics: data.topics.filter((x) => !x.courseId) },
  ];

  return (
    <div className="content">
      <div className="page" style={{ maxWidth: 1040 }}>
        <div className="row" style={{ marginTop: 16 }}>
          <h1 style={{ margin: 0, fontSize: 30 }}>📚 {t("Learning hub")}</h1>
          <span className="spacer" />
          <button className="btn" onClick={() => s.navigate({ name: "skills" })}>🧠 {t("Skills & evidence")}</button>
          <button className="btn primary" onClick={() => setEditCourse("new")}>＋ {t("Course")}</button>
        </div>
        <p className="muted small-text">{t("Courses break into topics. Mark a topic as reviewing to schedule spaced reviews; each answer moves the next review further out (or back to tomorrow if you forgot). Reviews are practice, not skill evidence.")}</p>

        <div style={{ display: "grid", gridTemplateColumns: "1fr 300px", gap: 20, alignItems: "start" }}>
          <div>
            <div className="h-section" style={{ marginTop: 8 }}>{t("Reviews due")} · {data.reviewsDue.length}</div>
            {data.reviewsDue.length === 0 ? (
              <div className="muted small-text">{t("Nothing to review today.")}</div>
            ) : (
              data.reviewsDue.map((x) => (
                <div key={x.id} className="card-box" style={{ padding: "10px 14px", marginBottom: 8 }}>
                  <div className="row">
                    <strong style={{ flex: 1 }}>{x.title}</strong>
                    {x.courseTitle && <span className="small-text faint">{x.courseTitle}</span>}
                    {x.reviewDue && x.reviewDue < data.today && <span className="chip c-red">{t("overdue")}</span>}
                  </div>
                  <div className="small-text muted" style={{ margin: "2px 0 8px" }}>{t("Try to recall it before opening your notes. How did it go?")}</div>
                  <div className="row wrap">
                    {QUALITY.map((b) => (
                      <button key={b.q} className={`btn small${b.q === 4 ? " primary" : ""}`} title={t(b.hint)} onClick={() => growth.learning.review(x.id, b.q).then((r) => (s.toast(t("Next review {date}", { date: r.reviewDue ?? "—" })), load())).catch(s.fail)}>
                        {t(b.label)}
                      </button>
                    ))}
                  </div>
                </div>
              ))
            )}

            {data.courses.length === 0 && data.topics.length === 0 ? (
              <EmptyState icon="📚" title={t("No courses yet")} hint={t("Add a course, class or book you are studying, then list its topics.")}>
                <button className="btn primary" onClick={() => setEditCourse("new")}>＋ {t("Course")}</button>
              </EmptyState>
            ) : (
              groups.filter((g) => g.course || g.topics.length).map(({ course, topics }) => (
                <div key={course?.id ?? "none"}>
                  <div className="h-section">
                    <span>{course ? course.title : t("Topics without a course")}</span>
                    {course && <span className={`chip ${course.status === "active" ? "c-blue" : course.status === "done" ? "c-green" : "c-gray"}`}>{t(course.status)}</span>}
                    <span className="spacer" />
                    {course?.url && <button className="icon-btn small" aria-label={t("Open link")} onClick={() => api.system.openExternal(course.url)}>🔗</button>}
                    {course && <button className="btn small ghost" onClick={() => setEditCourse(course)}>{t("Edit")}</button>}
                    <button className="btn small ghost" onClick={() => setResourceFor({ courseId: course?.id })}>＋ {t("Resource")}</button>
                    <button className="btn small ghost" onClick={() => addTopic(course?.id ?? null)}>＋ {t("Topic")}</button>
                  </div>
                  {course && (course.provider || course.goalTitle || course.startDate || course.endDate) && (
                    <div className="row wrap small-text faint" style={{ marginTop: -4, marginBottom: 6, gap: 10 }}>
                      {course.provider && <span>{course.provider}</span>}
                      {(course.startDate || course.endDate) && <span>📅 {course.startDate ?? "…"} → {course.endDate ?? "…"}</span>}
                      {course.goalTitle && <button className="crumb small-text" onClick={() => s.navigate({ name: "goal", id: course.goalId! })}>🎯 {course.goalTitle}</button>}
                    </div>
                  )}
                  {topics.length === 0 && <div className="small-text muted">{t("No topics yet.")}</div>}
                  {topics.map((x) => <TopicRow key={x.id} topic={x} all={data.topics} byId={byId} skills={skills} resources={data.resources.filter((r) => r.topicId === x.id)} reload={load} onResource={() => setResourceFor({ courseId: course?.id, topicId: x.id })} />)}
                  {data.resources.filter((r) => r.courseId === (course?.id ?? null) && !r.topicId).map((r) => <ResourceRow key={r.id} r={r} reload={load} />)}
                </div>
              ))
            )}
            {!data.topics.some((x) => !x.courseId) && data.courses.length > 0 && (
              <button className="btn small ghost" style={{ marginTop: 12 }} onClick={() => addTopic(null)}>＋ {t("Topic without a course")}</button>
            )}
          </div>

          <aside>
            <div className="card-box" style={{ padding: "12px 14px" }}>
              <h3>{t("Progress")}</h3>
              {(Object.keys(TOPIC_STATUS) as Topic["status"][]).map((k) => {
                const n = data.topics.filter((x) => x.status === k).length;
                return (
                  <div key={k} className="row small-text" style={{ marginBottom: 4 }}>
                    <span style={{ width: 16 }}>{TOPIC_STATUS[k].icon}</span>
                    <span style={{ flex: 1 }}>{t(TOPIC_STATUS[k].label)}</span>
                    <strong>{n}</strong>
                  </div>
                );
              })}
            </div>
            <div className="card-box" style={{ padding: "12px 14px" }}>
              <h3>{t("Weak spots")}</h3>
              <p className="desc">{t("Topics you forgot most often in reviews.")}</p>
              {data.weakTopics.length === 0 ? <div className="small-text muted">{t("None yet.")}</div> : data.weakTopics.map((x) => {
                const max = data.weakTopics[0].errorCount || 1;
                return (
                  <div key={x.id} className="small-text" style={{ marginBottom: 6 }}>
                    <div className="row"><span style={{ flex: 1 }}>{x.title}</span><span className="muted">{t("{n} misses", { n: x.errorCount })}</span></div>
                    <div style={{ height: 6, borderRadius: 6, background: "var(--bg-active)", overflow: "hidden" }}>
                      <div style={{ width: `${(x.errorCount / max) * 100}%`, height: "100%", background: "var(--series-1)", borderRadius: 6 }} />
                    </div>
                  </div>
                );
              })}
            </div>
          </aside>
        </div>
      </div>
      {editCourse && <CourseDialog course={editCourse === "new" ? null : editCourse} goals={goals} onClose={() => setEditCourse(null)} onSaved={() => (setEditCourse(null), load())} />}
      {resourceFor && <ResourceDialog preset={resourceFor} onClose={() => setResourceFor(null)} onSaved={() => (setResourceFor(null), load())} />}
    </div>
  );
}

function TopicRow({ topic: x, all, byId, skills, resources, reload, onResource }: { topic: Topic; all: Topic[]; byId: Map<string, Topic>; skills: Skill[]; resources: Resource[]; reload: () => void; onResource: () => void }) {
  const s = useStore();
  const { t } = useI18n();
  const pre = useAnchor();
  const save = (patch: Record<string, unknown>) => growth.learning.saveTopic(x.id, patch).then(reload).catch(s.fail);
  const pending = x.prerequisites.map((id) => byId.get(id)).filter((p): p is Topic => !!p && p.status !== "mastered" && p.status !== "reviewing");
  const st = TOPIC_STATUS[x.status];
  return (
    <div className="task-row" style={{ cursor: "default", alignItems: "flex-start" }}>
      <span style={{ width: 18, paddingTop: 2 }} aria-hidden>{st.icon}</span>
      <div className="t">
        <div className="row">
          <input className="input ghost-input" style={{ flex: 1, fontWeight: 500 }} defaultValue={x.title} aria-label={t("Topic")} onBlur={(e) => e.target.value.trim() && e.target.value !== x.title && save({ title: e.target.value })} />
          <select className="select" style={{ width: 130, height: 26 }} value={x.status} aria-label={t("Status")} onChange={(e) => save({ status: e.target.value })}>
            {(Object.keys(TOPIC_STATUS) as Topic["status"][]).map((k) => <option key={k} value={k}>{t(TOPIC_STATUS[k].label)}</option>)}
          </select>
          <select className="select" style={{ width: 140, height: 26 }} value={x.skillId ?? ""} aria-label={t("Skill")} onChange={(e) => save({ skillId: e.target.value || null })}>
            <option value="">{t("No skill")}</option>
            {skills.map((k) => <option key={k.id} value={k.id}>🧠 {k.name}</option>)}
          </select>
        </div>
        <div className="task-meta">
          {x.reviewDue && <span>↻ {t("next review")} {x.reviewDue}</span>}
          {x.lastReviewedAt && <span className="faint">{t("last")} {x.lastReviewedAt}</span>}
          <button className="crumb" onClick={pre.toggle}>⛓ {x.prerequisites.length ? x.prerequisites.map((id) => byId.get(id)?.title ?? "?").join(", ") : t("prerequisites")}</button>
          {pending.length > 0 && <span style={{ color: "var(--warning)" }}>▲ {t("learn {names} first", { names: pending.map((p) => p.title).join(", ") })}</span>}
          <button className="crumb" onClick={onResource}>＋ {t("resource")}</button>
        </div>
        {resources.map((r) => <ResourceRow key={r.id} r={r} reload={reload} compact />)}
      </div>
      <button className="icon-btn small" aria-label={t("Delete topic")} onClick={async () => { if (await s.confirm({ title: t("Delete this topic?"), danger: true, confirmLabel: t("Delete") })) growth.learning.deleteTopic(x.id).then(reload).catch(s.fail); }}><Icon name="trash" size={13} /></button>
      {pre.anchor && (
        <Popover anchor={pre.anchor} onClose={pre.close} width={280}>
          <div style={{ padding: 8, maxHeight: 320, overflow: "auto" }}>
            <div className="menu-label" style={{ paddingLeft: 0 }}>{t("Learn these first")}</div>
            {all.filter((o) => o.id !== x.id).map((o) => (
              <label key={o.id} className="row small-text" style={{ padding: "3px 0" }}>
                <input type="checkbox" checked={x.prerequisites.includes(o.id)} onChange={(e) => save({ prerequisites: e.target.checked ? [...x.prerequisites, o.id] : x.prerequisites.filter((id) => id !== o.id) })} />
                <span>{o.title}</span>{o.courseTitle && <span className="faint">· {o.courseTitle}</span>}
              </label>
            ))}
          </div>
        </Popover>
      )}
    </div>
  );
}

function ResourceRow({ r, reload, compact }: { r: Resource; reload: () => void; compact?: boolean }) {
  const s = useStore();
  const { t } = useI18n();
  return (
    <div className="row small-text" style={{ padding: compact ? "2px 0" : "4px 8px" }}>
      <input type="checkbox" checked={r.done} aria-label={t("Done")} onChange={(e) => growth.learning.saveResource(r.id, { done: e.target.checked }).then(reload).catch(s.fail)} />
      <span>{RESOURCE_ICON[r.kind]}</span>
      {r.url ? <a href="#" onClick={(e) => (e.preventDefault(), api.system.openExternal(r.url))} style={{ textDecoration: r.done ? "line-through" : undefined }}>{r.title}</a>
        : r.pageId ? <a href="#" onClick={(e) => (e.preventDefault(), s.navigate({ name: "page", id: r.pageId! }))}>{r.title}</a>
        : <span style={{ textDecoration: r.done ? "line-through" : undefined }}>{r.title}</span>}
      <span className="spacer" />
      <button className="icon-btn small" aria-label={t("Delete resource")} onClick={() => growth.learning.deleteResource(r.id).then(reload).catch(s.fail)}><Icon name="close" size={12} /></button>
    </div>
  );
}

function CourseDialog({ course, goals, onClose, onSaved }: { course: Course | null; goals: Goal[]; onClose: () => void; onSaved: () => void }) {
  const s = useStore();
  const { t } = useI18n();
  const [f, setF] = useState({ title: course?.title ?? "", provider: course?.provider ?? "", url: course?.url ?? "", status: course?.status ?? "active", goalId: course?.goalId ?? "", startDate: course?.startDate ?? "", endDate: course?.endDate ?? "", notes: course?.notes ?? "" });
  const set = (k: keyof typeof f, v: string) => setF((x) => ({ ...x, [k]: v }));
  const remove = async () => {
    if (course && (await s.confirm({ title: t("Delete this course?"), message: t("Its topics stay, without a course."), danger: true, confirmLabel: t("Delete") }))) growth.learning.deleteCourse(course.id).then(onSaved).catch(s.fail);
  };
  return (
    <Modal title={course ? t("Edit course") : t("New course")} onClose={onClose} footer={<>
      {course && <button className="btn danger" onClick={remove}>{t("Delete")}</button>}
      <span className="spacer" />
      <button className="btn" onClick={onClose}>{t("Cancel")}</button>
      <button className="btn primary" onClick={() => growth.learning.saveCourse(course?.id ?? null, { ...f, goalId: f.goalId || null, startDate: f.startDate || null, endDate: f.endDate || null }).then(onSaved).catch(s.fail)}>{t("Save")}</button>
    </>}>
      <div className="field"><label>{t("Name")}</label><input className="input" autoFocus value={f.title} placeholder={t("e.g. CS50, IELTS prep, Calculus I")} onChange={(e) => set("title", e.target.value)} /></div>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "0 12px" }}>
        <div className="field"><label>{t("Provider")}</label><input className="input" value={f.provider} placeholder={t("School, platform, author")} onChange={(e) => set("provider", e.target.value)} /></div>
        <div className="field"><label>{t("Status")}</label><select className="select" value={f.status} onChange={(e) => set("status", e.target.value)}>{COURSE_STATUS.map((k) => <option key={k} value={k}>{t(k)}</option>)}</select></div>
        <div className="field"><label>{t("Start")}</label><input className="input" type="date" value={f.startDate} onChange={(e) => set("startDate", e.target.value)} /></div>
        <div className="field"><label>{t("End")}</label><input className="input" type="date" value={f.endDate} onChange={(e) => set("endDate", e.target.value)} /></div>
      </div>
      <div className="field"><label>{t("Link")}</label><input className="input" value={f.url} placeholder="https://" onChange={(e) => set("url", e.target.value)} /></div>
      <div className="field"><label>{t("For goal (optional)")}</label>
        <select className="select" value={f.goalId} onChange={(e) => set("goalId", e.target.value)}><option value="">—</option>{goals.map((g) => <option key={g.id} value={g.id}>{g.title}</option>)}</select>
      </div>
      <div className="field"><label>{t("Notes")}</label><textarea className="input" rows={3} value={f.notes} onChange={(e) => set("notes", e.target.value)} /></div>
    </Modal>
  );
}

function ResourceDialog({ preset, onClose, onSaved }: { preset: { courseId?: string; topicId?: string }; onClose: () => void; onSaved: () => void }) {
  const s = useStore();
  const { t } = useI18n();
  const [f, setF] = useState({ title: "", kind: "link", url: "" });
  return (
    <Modal title={t("Add resource")} onClose={onClose} footer={<><button className="btn" onClick={onClose}>{t("Cancel")}</button><button className="btn primary" onClick={() => growth.learning.saveResource(null, { ...f, courseId: preset.courseId ?? null, topicId: preset.topicId ?? null }).then(onSaved).catch(s.fail)}>{t("Add")}</button></>}>
      <div className="field"><label>{t("Title")}</label><input className="input" autoFocus value={f.title} onChange={(e) => setF({ ...f, title: e.target.value })} /></div>
      <div style={{ display: "grid", gridTemplateColumns: "140px 1fr", gap: "0 12px" }}>
        <div className="field"><label>{t("Type")}</label><select className="select" value={f.kind} onChange={(e) => setF({ ...f, kind: e.target.value })}>{Object.keys(RESOURCE_ICON).map((k) => <option key={k} value={k}>{RESOURCE_ICON[k as Resource["kind"]]} {t(k)}</option>)}</select></div>
        <div className="field"><label>{t("Link (optional)")}</label><input className="input" value={f.url} placeholder="https://" onChange={(e) => setF({ ...f, url: e.target.value })} /></div>
      </div>
    </Modal>
  );
}
