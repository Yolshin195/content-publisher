/* Content Publisher — клиентская логика интерфейса (Alpine.js-компоненты).
   Подключается до Alpine: компоненты регистрируются в событии alpine:init. */
'use strict';

const APP_TZ = document.querySelector('meta[name="app-tz"]')?.content || 'UTC';
const DEFAULT_TIME = '09:00';

// ─────────────────────────── HTTP ───────────────────────────

async function api(method, url, body) {
  const opts = { method, headers: { Accept: 'application/json' }, credentials: 'same-origin' };
  if (body !== undefined) {
    opts.headers['Content-Type'] = 'application/json';
    opts.body = JSON.stringify(body);
  }
  const resp = await fetch(url, opts);
  if (resp.status === 401) {
    location.href = '/login?next=' + encodeURIComponent(location.pathname + location.search);
    throw new Error('Нужно войти заново');
  }
  if (resp.status === 204) return null;
  const text = await resp.text();
  let data = null;
  try { data = text ? JSON.parse(text) : null; } catch (_) { /* не JSON */ }
  if (!resp.ok) {
    const msg = data?.error?.message || text || resp.statusText;
    throw new Error(msg.replace(/^validation error: /, ''));
  }
  return data;
}

function toast(message, kind = 'ok') {
  window.dispatchEvent(new CustomEvent('toast', { detail: { message, kind } }));
}

function openPost(id) { window.dispatchEvent(new CustomEvent('open-post', { detail: { id } })); }
function newPost(opts = {}) { window.dispatchEvent(new CustomEvent('new-post', { detail: opts })); }

async function copyText(text, what = 'Скопировано') {
  try {
    await navigator.clipboard.writeText(text);
    toast(what);
  } catch (_) {
    toast('Не удалось скопировать', 'error');
  }
}

// ─────────────────────── Часовой пояс автора ───────────────────────
// Все даты в интерфейсе показываются и вводятся в APP_TIMEZONE, а не в поясе браузера.

const pad = (n) => String(n).padStart(2, '0');
const _partsFmt = new Intl.DateTimeFormat('en-US', {
  timeZone: APP_TZ, hourCycle: 'h23', year: 'numeric', month: '2-digit', day: '2-digit',
  hour: '2-digit', minute: '2-digit', second: '2-digit',
});

function tzParts(date) {
  const p = {};
  for (const { type, value } of _partsFmt.formatToParts(date)) p[type] = Number(value);
  return p;
}

function tzOffsetMs(date) {
  const p = tzParts(date);
  const asUtc = Date.UTC(p.year, p.month - 1, p.day, p.hour, p.minute, p.second);
  return asUtc - Math.floor(date.getTime() / 1000) * 1000;
}

/** ISO (UTC) → 'YYYY-MM-DDTHH:mm' в поясе автора (для <input type=datetime-local>). */
function isoToLocalInput(iso) {
  if (!iso) return '';
  const p = tzParts(new Date(iso));
  return `${p.year}-${pad(p.month)}-${pad(p.day)}T${pad(p.hour)}:${pad(p.minute)}`;
}

/** 'YYYY-MM-DDTHH:mm' в поясе автора → ISO UTC. */
function localInputToIso(value) {
  if (!value) return null;
  const [d, t = '00:00'] = value.split('T');
  const [y, m, day] = d.split('-').map(Number);
  const [hh, mm] = t.split(':').map(Number);
  const guess = Date.UTC(y, m - 1, day, hh, mm);
  let ts = guess - tzOffsetMs(new Date(guess));
  const second = guess - tzOffsetMs(new Date(ts));
  if (second !== ts) ts = second; // переход на летнее/зимнее время
  return new Date(ts).toISOString();
}

function todayLocal() {
  const p = tzParts(new Date());
  return `${p.year}-${pad(p.month)}-${pad(p.day)}`;
}

const _dtFmt = new Intl.DateTimeFormat('ru-RU', { timeZone: APP_TZ, day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' });
const _dtFmtYear = new Intl.DateTimeFormat('ru-RU', { timeZone: APP_TZ, day: 'numeric', month: 'short', year: 'numeric', hour: '2-digit', minute: '2-digit' });
const _timeFmt = new Intl.DateTimeFormat('ru-RU', { timeZone: APP_TZ, hour: '2-digit', minute: '2-digit' });

function fmtDateTime(iso) {
  if (!iso) return '';
  const d = new Date(iso);
  return (tzParts(d).year === tzParts(new Date()).year ? _dtFmt : _dtFmtYear).format(d);
}
function fmtTime(iso) { return iso ? _timeFmt.format(new Date(iso)) : ''; }

function fmtRelative(iso) {
  if (!iso) return '';
  const s = (Date.now() - new Date(iso).getTime()) / 1000;
  if (s < 0) return fmtDateTime(iso);
  if (s < 60) return 'только что';
  if (s < 3600) return `${Math.floor(s / 60)} мин назад`;
  if (s < 86400) return `${Math.floor(s / 3600)} ч назад`;
  if (s < 172800) return 'вчера';
  if (s < 604800) return `${Math.floor(s / 86400)} дн назад`;
  return fmtDateTime(iso);
}

// ─────────────────────── Разметка Telegram ───────────────────────

const TG_ALLOWED = new Set(['b', 'strong', 'i', 'em', 'u', 'ins', 's', 'strike', 'del', 'a', 'code', 'pre', 'blockquote', 'tg-spoiler', 'span', 'tg-emoji']);
const TG_MESSAGE_LIMIT = 4096;
const TG_CAPTION_LIMIT = 1024;

/** Безопасный HTML для превью: только теги Telegram и безопасные атрибуты. */
function tgSanitize(html) {
  const doc = new DOMParser().parseFromString(`<body>${html || ''}</body>`, 'text/html');
  const out = document.createElement('div');
  const walk = (src, dst) => {
    for (const node of src.childNodes) {
      if (node.nodeType === Node.TEXT_NODE) {
        dst.appendChild(document.createTextNode(node.textContent));
      } else if (node.nodeType === Node.ELEMENT_NODE) {
        const tag = node.tagName.toLowerCase();
        if (!TG_ALLOWED.has(tag)) { walk(node, dst); continue; }
        const el = document.createElement(tag === 'tg-spoiler' ? 'span' : tag);
        if (tag === 'tg-spoiler') el.className = 'tg-spoiler';
        if (tag === 'span' && node.classList.contains('tg-spoiler')) el.className = 'tg-spoiler';
        if (tag === 'a') {
          const href = node.getAttribute('href') || '';
          if (/^(https?:|tg:|mailto:)/i.test(href)) {
            el.setAttribute('href', href);
            el.setAttribute('target', '_blank');
            el.setAttribute('rel', 'noopener noreferrer');
          }
        }
        walk(node, el);
        dst.appendChild(el);
      }
    }
  };
  walk(doc.body, out);
  return out.innerHTML;
}

function tgPlain(html) {
  const doc = new DOMParser().parseFromString(`<body>${html || ''}</body>`, 'text/html');
  return doc.body.textContent || '';
}

/** Проблемы разметки — зеркало проверки на сервере (domain/telegram_html.rs). */
function tgProblems(title, html, hasCover) {
  const problems = [];
  const unsupported = new Set();
  const stack = [];
  const re = /<(\/?)([a-zA-Z][a-zA-Z0-9-]*)([^>]*)>/g;
  let m;
  const src = html || '';
  while ((m = re.exec(src))) {
    const [, closing, rawName, rest] = m;
    const name = rawName.toLowerCase();
    if (!TG_ALLOWED.has(name)) { unsupported.add(`<${name}>`); continue; }
    if (closing) {
      const open = stack.pop();
      if (open === undefined) problems.push(`лишний закрывающий тег </${name}>`);
      else if (open !== name) problems.push(`</${name}> закрывает незакрытый <${open}>`);
    } else if (!rest.trim().endsWith('/')) {
      stack.push(name);
    }
  }
  stack.forEach((t) => problems.push(`тег <${t}> не закрыт`));
  if (unsupported.size) {
    problems.unshift(`Telegram не поддерживает ${[...unsupported].join(', ')} — если «<» часть текста, пишите &lt;`);
  }
  if (/<(?![a-zA-Z/])/.test(src)) problems.push('неэкранированный «<» — пишите &lt;');
  if (/&(?![a-zA-Z]+;|#\d+;|#x[0-9a-fA-F]+;)/.test(src)) problems.push('неэкранированный «&» — пишите &amp;');
  const limit = hasCover ? TG_CAPTION_LIMIT : TG_MESSAGE_LIMIT;
  const len = (title || '').length + 2 + tgPlain(src).length;
  if (len > limit) problems.push(`слишком длинно: ${len} из ${limit} символов${hasCover ? ' (с обложкой — подпись к фото)' : ''}`);
  return problems;
}

function displayStatus(a) {
  if (!a) return { key: 'draft', label: 'Новый пост' };
  switch (a.state) {
    case 'suggested': return { key: 'suggested', label: 'Предложение' };
    case 'draft': return { key: 'draft', label: 'Черновик' };
    case 'rejected': return { key: 'rejected', label: 'Отклонён' };
    case 'archived': return { key: 'archived', label: 'В архиве' };
    default: {
      const st = a.publication?.status;
      if (st === 'published') return { key: 'published', label: 'Опубликован' };
      if (st === 'failed') return { key: 'failed', label: 'Ошибка публикации' };
      if (st === 'partial') return { key: 'partial', label: 'Опубликован частично' };
      return { key: 'scheduled', label: 'Запланирован' };
    }
  }
}

const TASK_STATUS = {
  pending: 'ждёт', publishing: 'публикуется', published: 'опубликован', failed: 'ошибка', cancelled: 'отменён',
};
const TASK_BADGE = {
  pending: 'st-scheduled', publishing: 'st-scheduled', published: 'st-published', failed: 'st-failed', cancelled: 'st-archived',
};

const REJECT_REASONS = ['Не моя тема', 'Уже было', 'Слабый текст', 'Не актуально', 'Слишком длинно'];

// ─────────────────────── Тултипы ───────────────────────

(function tooltips() {
  let tip;
  const show = (el, x, y) => {
    if (!tip) { tip = document.createElement('div'); tip.id = 'tooltip'; document.body.appendChild(tip); }
    tip.textContent = el.dataset.tip;
    tip.style.display = 'block';
    const r = tip.getBoundingClientRect();
    tip.style.left = Math.min(window.innerWidth - r.width - 8, Math.max(8, x - r.width / 2)) + 'px';
    tip.style.top = Math.max(8, y - r.height - 12) + 'px';
  };
  const hide = () => { if (tip) tip.style.display = 'none'; };
  document.addEventListener('mousemove', (e) => {
    const el = e.target.closest?.('[data-tip]:not([data-tip=""])');
    el ? show(el, e.clientX, e.clientY) : hide();
  });
  document.addEventListener('focusin', (e) => {
    const el = e.target.closest?.('[data-tip]:not([data-tip=""])');
    if (el) { const r = el.getBoundingClientRect(); show(el, r.left + r.width / 2, r.top); }
  });
  document.addEventListener('focusout', hide);
  document.addEventListener('scroll', hide, true);
})();

// ─────────────────────── Тема ───────────────────────

function setTheme(theme) {
  document.documentElement.dataset.theme = theme;
  try { localStorage.setItem('theme', theme); } catch (_) { /* приватный режим */ }
}

// ─────────────────────── Компоненты Alpine ───────────────────────

function emptyForm() {
  return { title: '', content_html: '', excerpt: '', tags: [], cover_image_url: '', scheduled_local: '', rationale: '', sources: [] };
}

/** Редактор поста: и выезжающая панель (mode=drawer), и отдельная страница (mode=page). */
function postEditor(opts = {}) {
  return {
    mode: opts.mode || 'drawer',
    open: opts.mode === 'page',
    loading: false,
    busy: false,
    changed: false,
    id: opts.id || null,
    a: null,
    form: emptyForm(),
    original: '',
    targets: [],
    selectedTargets: [],
    tasks: [],
    videoLinks: [],
    newVideo: { platform: 'youtube', url: '' },
    tagInput: '',
    showReject: false,
    rejectReason: '',
    reasons: REJECT_REASONS,
    pane: 'edit',
    tz: APP_TZ,

    async init() {
      if (this.mode === 'page') {
        await this.loadTargets();
        if (this.id) await this.load(this.id);
        else this.startNew({ date: opts.date });
      } else {
        window.addEventListener('open-post', (e) => this.openDrawer(e.detail.id));
        window.addEventListener('new-post', (e) => this.newDrawer(e.detail || {}));
      }
      window.addEventListener('beforeunload', (e) => {
        if (this.open && this.dirty) { e.preventDefault(); e.returnValue = ''; }
      });
    },

    get dirty() { return this.original !== '' && JSON.stringify(this.form) !== this.original; },
    get isNew() { return !this.id; },
    get status() { return displayStatus(this.a); },
    get preview() { return tgSanitize(this.form.content_html); },
    get hasCover() { return !!(this.form.cover_image_url || '').trim(); },
    get problems() { return tgProblems(this.form.title, this.form.content_html, this.hasCover); },
    get length() { return (this.form.title || '').length + 2 + tgPlain(this.form.content_html).length; },
    get limit() { return this.hasCover ? TG_CAPTION_LIMIT : TG_MESSAGE_LIMIT; },
    get lengthPct() { return Math.min(100, Math.round((this.length / this.limit) * 100)); },
    get canSchedule() { return !this.a || !['rejected', 'archived'].includes(this.a.state); },
    get isScheduled() { return this.a?.state === 'scheduled'; },
    get published() { return (this.a?.publication?.published || 0) > 0; },
    get previewTime() {
      return this.form.scheduled_local ? this.form.scheduled_local.slice(11, 16) : fmtTime(new Date().toISOString());
    },
    get scheduledLabel() {
      return this.form.scheduled_local ? fmtDateTime(localInputToIso(this.form.scheduled_local)) : '';
    },
    targetName(id) { return this.targets.find((t) => t.id === id)?.display_name || 'Площадка'; },
    taskLabel(s) { return TASK_STATUS[s] || s; },
    taskBadge(s) { return TASK_BADGE[s] || 'badge-neutral'; },
    fmtDateTime,
    fmtRelative,

    async loadTargets() {
      try {
        this.targets = await api('GET', '/api/targets');
      } catch (e) { toast(e.message, 'error'); }
    },

    reset() {
      this.a = null;
      this.form = emptyForm();
      this.tasks = [];
      this.videoLinks = [];
      this.showReject = false;
      this.rejectReason = '';
      this.tagInput = '';
      this.pane = 'edit';
      this.selectedTargets = this.targets.filter((t) => t.is_active).map((t) => t.id);
    },

    async openDrawer(id) {
      if (this.open && this.dirty && !confirm('Есть несохранённые изменения. Открыть другой пост?')) return;
      this.open = true;
      document.documentElement.classList.add('overflow-hidden');
      if (!this.targets.length) await this.loadTargets();
      await this.load(id);
    },

    async newDrawer({ date }) {
      if (this.open && this.dirty && !confirm('Есть несохранённые изменения. Создать новый пост?')) return;
      this.open = true;
      document.documentElement.classList.add('overflow-hidden');
      if (!this.targets.length) await this.loadTargets();
      this.startNew({ date });
    },

    startNew({ date } = {}) {
      this.id = null;
      this.reset();
      if (date) this.form.scheduled_local = `${date}T${DEFAULT_TIME}`;
      this.original = JSON.stringify(this.form);
      this.$nextTick(() => this.$refs.title?.focus());
    },

    close() {
      if (this.mode === 'page') { history.length > 1 ? history.back() : (location.href = '/posts'); return; }
      if (this.dirty && !confirm('Есть несохранённые изменения. Закрыть без сохранения?')) return;
      this.open = false;
      document.documentElement.classList.remove('overflow-hidden');
      if (this.changed) location.reload();
    },

    fill(a) {
      this.a = a;
      this.id = a.id;
      this.form = {
        title: a.title || '',
        content_html: a.content_html || '',
        excerpt: a.excerpt || '',
        tags: [...(a.tags || [])],
        cover_image_url: a.cover_image_url || '',
        scheduled_local: isoToLocalInput(a.scheduled_at),
        rationale: a.rationale || '',
        sources: (a.sources || []).map((s) => ({ ...s })),
      };
      this.videoLinks = a.video_links || [];
      this.original = JSON.stringify(this.form);
    },

    async load(id) {
      this.loading = true;
      this.reset();
      this.id = id;
      try {
        const [a, tasks] = await Promise.all([api('GET', `/api/articles/${id}`), api('GET', `/api/articles/${id}/tasks`)]);
        this.fill(a);
        this.tasks = tasks;
        const assigned = tasks.filter((t) => t.status !== 'cancelled').map((t) => t.target_id);
        if (assigned.length) this.selectedTargets = assigned;
      } catch (e) {
        toast(e.message, 'error');
      } finally {
        this.loading = false;
      }
    },

    async refresh() {
      if (this.id) {
        const [a, tasks] = await Promise.all([api('GET', `/api/articles/${this.id}`), api('GET', `/api/articles/${this.id}/tasks`)]);
        const keepDirty = this.dirty;
        const form = this.form;
        this.fill(a);
        if (keepDirty) this.form = form;
        this.tasks = tasks;
      }
    },

    payload() {
      const f = this.form;
      return {
        title: f.title.trim(),
        content_html: f.content_html,
        excerpt: f.excerpt.trim() || null,
        tags: f.tags,
        cover_image_url: f.cover_image_url.trim() || null,
        rationale: f.rationale.trim() || null,
        sources: f.sources.filter((s) => s.url.trim()),
      };
    },

    async save({ quiet = false } = {}) {
      if (!this.form.title.trim()) {
        toast('Добавьте заголовок', 'error');
        this.$refs.title?.focus();
        return false;
      }
      this.busy = true;
      try {
        const body = this.payload();
        const prev = this.original ? JSON.parse(this.original).scheduled_local : '';
        let a;
        if (this.isNew) {
          a = await api('POST', '/api/articles', { ...body, scheduled_at: localInputToIso(this.form.scheduled_local) });
          if (this.mode === 'page') history.replaceState(null, '', `/articles/${a.id}`);
        } else {
          if (this.form.scheduled_local !== prev) body.scheduled_at = localInputToIso(this.form.scheduled_local);
          a = await api('PATCH', `/api/articles/${this.id}`, body);
        }
        this.fill(a);
        this.changed = true;
        if (!quiet) toast('Сохранено');
        return true;
      } catch (e) {
        toast(e.message, 'error');
        return false;
      } finally {
        this.busy = false;
      }
    },

    async ensureSaved() {
      if (this.isNew || this.dirty) return this.save({ quiet: true });
      return true;
    },

    async run(fn, okMessage) {
      this.busy = true;
      try {
        await fn();
        this.changed = true;
        await this.refresh();
        if (okMessage) toast(okMessage);
      } catch (e) {
        toast(e.message, 'error');
      } finally {
        this.busy = false;
      }
    },

    async schedule() {
      if (!this.form.scheduled_local) {
        toast('Выберите дату и время публикации', 'error');
        this.$refs.when?.focus();
        return;
      }
      if (localInputToIso(this.form.scheduled_local) < new Date().toISOString()
        && !confirm('Время уже прошло — пост уйдёт сразу. Продолжить?')) return;
      if (!(await this.ensureSaved())) return;
      await this.run(
        () => api('POST', `/api/articles/${this.id}/schedule`, {
          scheduled_at: localInputToIso(this.form.scheduled_local),
          target_ids: this.selectedTargets,
        }),
        `Запланировано на ${this.scheduledLabel}`,
      );
    },

    async publishNow() {
      if (!confirm('Опубликовать прямо сейчас?')) return;
      if (!(await this.ensureSaved())) return;
      await this.run(async () => {
        const tasks = await api('POST', `/api/articles/${this.id}/publish-now`, { target_ids: this.selectedTargets });
        const failed = tasks.filter((t) => t.status === 'failed' || (t.status === 'pending' && t.last_error));
        if (failed.length) throw new Error(`Не удалось: ${failed.map((t) => t.last_error).join('; ')}`);
      }, 'Опубликовано');
    },

    async unschedule() {
      await this.run(() => api('POST', `/api/articles/${this.id}/unschedule`), 'Снято с публикации — теперь это черновик');
    },

    async accept() {
      if (!(await this.ensureSaved())) return;
      await this.run(() => api('POST', `/api/articles/${this.id}/accept`, {}), 'Принято в черновики');
    },

    async acceptAndSchedule() {
      if (!(await this.ensureSaved())) return;
      if (this.a?.state === 'suggested') {
        try { await api('POST', `/api/articles/${this.id}/accept`, {}); } catch (e) { toast(e.message, 'error'); return; }
      }
      await this.schedule();
    },

    async reject() {
      await this.run(() => api('POST', `/api/articles/${this.id}/reject`, { feedback: this.rejectReason.trim() || null }), 'Отклонено');
      this.showReject = false;
    },

    async restore() { await this.run(() => api('POST', `/api/articles/${this.id}/restore`), 'Возвращено в предложения'); },
    async archive() { await this.run(() => api('POST', `/api/articles/${this.id}/archive`), 'Перенесено в архив'); },

    async remove() {
      if (!confirm('Удалить пост? Это действие нельзя отменить.')) return;
      try {
        await api('DELETE', `/api/articles/${this.id}`);
        toast('Пост удалён');
        this.changed = true;
        this.original = JSON.stringify(this.form);
        if (this.mode === 'page') location.href = '/posts';
        else this.close();
      } catch (e) { toast(e.message, 'error'); }
    },

    async retry(t) { await this.run(() => api('POST', `/api/articles/${this.id}/targets/${t.target_id}/retry`), 'Повторная попытка поставлена в очередь'); },
    async cancelTask(t) { await this.run(() => api('DELETE', `/api/articles/${this.id}/targets/${t.target_id}`), 'Публикация в площадку отменена'); },

    async addVideo() {
      const url = this.newVideo.url.trim();
      if (!url) return;
      if (!(await this.ensureSaved())) return;
      try {
        const link = await api('POST', `/api/articles/${this.id}/video-links`, { platform: this.newVideo.platform, url, is_primary: !this.videoLinks.length });
        this.videoLinks.push(link);
        this.newVideo.url = '';
        this.changed = true;
      } catch (e) { toast(e.message, 'error'); }
    },

    async removeVideo(link) {
      try {
        await api('DELETE', `/api/articles/${this.id}/video-links/${link.id}`);
        this.videoLinks = this.videoLinks.filter((l) => l.id !== link.id);
        this.changed = true;
      } catch (e) { toast(e.message, 'error'); }
    },

    addTag() {
      const parts = this.tagInput.split(/[,\s]+/).map((t) => t.replace(/^#/, '').trim().toLowerCase()).filter(Boolean);
      for (const t of parts) if (!this.form.tags.includes(t)) this.form.tags.push(t);
      this.tagInput = '';
    },
    removeTag(t) { this.form.tags = this.form.tags.filter((x) => x !== t); },
    addSource() { this.form.sources.push({ title: '', url: '' }); },

    /** Обернуть выделение в тег (кнопки панели и горячие клавиши). */
    wrap(tag) {
      const ta = this.$refs.body;
      if (!ta) return;
      const { selectionStart: s, selectionEnd: e, value } = ta;
      const sel = value.slice(s, e);
      let open = `<${tag}>`;
      const close = `</${tag}>`;
      if (tag === 'a') {
        const url = prompt('Адрес ссылки', 'https://');
        if (!url) return;
        open = `<a href="${url.replace(/"/g, '&quot;')}">`;
      }
      const text = open + (sel || (tag === 'a' ? 'ссылка' : '')) + close;
      ta.focus();
      if (!document.execCommand('insertText', false, text)) ta.setRangeText(text, s, e, 'end');
      this.form.content_html = ta.value;
      if (!sel) {
        const cursor = s + open.length;
        ta.setSelectionRange(cursor, cursor + (tag === 'a' ? 6 : 0));
      }
    },

    onKey(e) {
      if (!this.open) return;
      const mod = e.ctrlKey || e.metaKey;
      if (mod && e.key.toLowerCase() === 's') { e.preventDefault(); this.save(); return; }
      if (e.key === 'Escape' && this.mode === 'drawer' && this.open) {
        if (this.showReject) { this.showReject = false; return; }
        this.close();
        return;
      }
      if (mod && e.target === this.$refs.body) {
        const map = { b: 'b', i: 'i', u: 'u', k: 'a' };
        const tag = map[e.key.toLowerCase()];
        if (tag) { e.preventDefault(); this.wrap(tag); }
      }
    },
  };
}

/** Входящие: быстрые действия над карточками предложений. */
function inboxPage() {
  return {
    rejectId: null,
    rejectTitle: '',
    rejectReason: '',
    reasons: REJECT_REASONS,
    hidden: {},
    async accept(id) {
      try {
        await api('POST', `/api/articles/${id}/accept`, {});
        this.hidden[id] = true;
        toast('Принято в черновики — запланируйте публикацию в «Плане»');
      } catch (e) { toast(e.message, 'error'); }
    },
    askReject(id, title) {
      this.rejectId = id;
      this.rejectTitle = title;
      this.rejectReason = '';
      this.$nextTick(() => this.$refs.reason?.focus());
    },
    async confirmReject() {
      const id = this.rejectId;
      try {
        await api('POST', `/api/articles/${id}/reject`, { feedback: this.rejectReason.trim() || null });
        this.hidden[id] = true;
        this.rejectId = null;
        toast('Отклонено. Claude учтёт причину в следующих предложениях');
      } catch (e) { toast(e.message, 'error'); }
    },
    async restore(id) {
      try {
        await api('POST', `/api/articles/${id}/restore`);
        location.reload();
      } catch (e) { toast(e.message, 'error'); }
    },
  };
}

/** План: перетаскивание постов между днями и из списка «Без даты». */
function calendarPage() {
  return {
    dragId: null,
    dragTime: DEFAULT_TIME,
    overDate: null,
    dragStart(e, id, time) {
      this.dragId = id;
      this.dragTime = time || DEFAULT_TIME;
      e.dataTransfer.effectAllowed = 'move';
      e.dataTransfer.setData('text/plain', id);
    },
    dragOver(e, date) {
      if (!this.dragId) return;
      e.preventDefault();
      e.dataTransfer.dropEffect = 'move';
      this.overDate = date;
    },
    dragEnd() { this.dragId = null; this.overDate = null; },
    async drop(e, date) {
      e.preventDefault();
      const id = this.dragId;
      this.dragEnd();
      if (!id) return;
      try {
        await api('PATCH', `/api/articles/${id}`, { scheduled_at: localInputToIso(`${date}T${this.dragTime}`) });
        toast('Дата публикации изменена');
        location.reload();
      } catch (err) { toast(err.message, 'error'); }
    },
  };
}

/** Справочник промптов: дерево, редактор, история с diff, посты по версиям. */
function promptsPage() {
  return {
    tree: [],
    expanded: {},
    filter: '',
    loadingTree: true,
    node: null,
    form: { key: '', title: '', description: '', body: '', change_note: '' },
    original: '',
    saving: false,
    tab: 'edit',
    resolved: null,
    showResolved: false,
    versions: [],
    diffFrom: null,
    diffTo: '',
    diff: [],
    viewVersion: null,
    posts: [],
    postsByVersion: [],
    dialog: null,
    dialogForm: { kind: 'prompt', key: '', title: '', parent_id: '' },
    moveTarget: '',
    dragId: null,
    dropId: null,

    async init() {
      try { this.expanded = JSON.parse(localStorage.getItem('prompts.expanded') || '{}'); } catch (_) { /* пусто */ }
      await this.loadTree();
      const key = new URLSearchParams(location.search).get('key');
      const first = key ? this.flat().find((n) => (n.key || '').toLowerCase() === key.toLowerCase()) : this.flat().find((n) => n.kind === 'prompt');
      if (first) await this.select(first.id, true);
      window.addEventListener('beforeunload', (e) => { if (this.dirty) { e.preventDefault(); e.returnValue = ''; } });
    },

    get dirty() { return this.node && this.original !== '' && JSON.stringify(this.form) !== this.original; },
    get isFolder() { return this.node?.kind === 'folder'; },
    get includes() {
      const out = [];
      const re = /\{\{>\s*([A-Za-z0-9_.-]+)\s*\}\}/g;
      let m;
      while ((m = re.exec(this.form.body || ''))) if (!out.includes(m[1])) out.push(m[1]);
      return out;
    },
    get folders() { return this.flat().filter((n) => n.kind === 'folder'); },
    get promptKeys() { return this.flat().filter((n) => n.kind === 'prompt').map((n) => n.key); },
    get stats() { return { lines: (this.form.body || '').split('\n').length, chars: (this.form.body || '').length }; },

    flat(nodes = this.tree, depth = 0, out = []) {
      for (const n of nodes) {
        out.push({ ...n, depth });
        this.flat(n.children || [], depth + 1, out);
      }
      return out;
    },

    /** Видимые строки дерева: учитывает свёрнутые папки и фильтр (при поиске всё раскрыто). */
    rows() {
      const q = this.filter.trim().toLowerCase();
      const matches = (n) => [n.key, n.title, n.description].some((v) => (v || '').toLowerCase().includes(q));
      const hasMatch = (n) => (n.children || []).some((c) => matches(c) || hasMatch(c));
      const out = [];
      const visit = (nodes, depth) => {
        for (const n of nodes) {
          if (q && !matches(n) && !hasMatch(n)) continue;
          const hasChildren = (n.children || []).length > 0;
          const open = q ? true : !!this.expanded[n.id];
          out.push({ ...n, depth, hasChildren, open });
          if (hasChildren && open) visit(n.children, depth + 1);
        }
      };
      visit(this.tree, 0);
      return out;
    },

    toggle(id) {
      this.expanded[id] = !this.expanded[id];
      try { localStorage.setItem('prompts.expanded', JSON.stringify(this.expanded)); } catch (_) { /* пусто */ }
    },

    async loadTree() {
      this.loadingTree = true;
      try { this.tree = await api('GET', '/api/prompts'); } catch (e) { toast(e.message, 'error'); }
      this.loadingTree = false;
    },

    expandTo(id) {
      const path = [];
      const find = (nodes, trail) => {
        for (const n of nodes) {
          if (n.id === id) { path.push(...trail); return true; }
          if (find(n.children || [], [...trail, n.id])) return true;
        }
        return false;
      };
      find(this.tree, []);
      path.forEach((p) => { this.expanded[p] = true; });
    },

    async select(id, initial = false) {
      if (!initial && this.dirty && !confirm('Есть несохранённые изменения. Перейти без сохранения?')) return;
      try {
        const n = await api('GET', `/api/prompts/${id}`);
        this.node = n;
        this.form = { key: n.key || '', title: n.title, description: n.description, body: n.body, change_note: '' };
        this.original = JSON.stringify(this.form);
        this.resolved = null;
        this.showResolved = false;
        this.viewVersion = null;
        this.expandTo(id);
        if (n.kind === 'folder') {
          this.tab = 'edit';
          if (!this.expanded[id]) this.toggle(id);
        }
        if (n.key) history.replaceState(null, '', `/prompts?key=${encodeURIComponent(n.key)}`);
        if (this.tab === 'history') await this.loadVersions();
        if (this.tab === 'posts') await this.loadPosts();
      } catch (e) { toast(e.message, 'error'); }
    },

    async setTab(tab) {
      this.tab = tab;
      if (tab === 'history') await this.loadVersions();
      if (tab === 'posts') await this.loadPosts();
    },

    async save() {
      if (!this.node || !this.dirty) return;
      this.saving = true;
      try {
        const body = { title: this.form.title, description: this.form.description, body: this.form.body, change_note: this.form.change_note || null };
        if (!this.isFolder) body.key = this.form.key;
        const prevVersion = this.node.version;
        const n = await api('PATCH', `/api/prompts/${this.node.id}`, body);
        this.node = n;
        this.form = { key: n.key || '', title: n.title, description: n.description, body: n.body, change_note: '' };
        this.original = JSON.stringify(this.form);
        this.resolved = null;
        toast(n.version !== prevVersion ? `Сохранено как версия ${n.version}` : 'Сохранено');
        await this.loadTree();
        if (this.tab === 'history') await this.loadVersions();
      } catch (e) { toast(e.message, 'error'); }
      this.saving = false;
    },

    revert() { this.form = JSON.parse(this.original); },

    async toggleResolved() {
      this.showResolved = !this.showResolved;
      if (this.showResolved) {
        try {
          if (this.dirty) toast('Показан сохранённый текст — несохранённые правки не учтены', 'info');
          this.resolved = await api('GET', `/api/prompts/by-key/${encodeURIComponent(this.node.key)}`);
        } catch (e) { toast(e.message, 'error'); this.showResolved = false; }
      }
    },

    async loadVersions() {
      if (!this.node || this.isFolder) return;
      try {
        this.versions = await api('GET', `/api/prompts/${this.node.id}/versions`);
        this.diffTo = '';
        this.diffFrom = this.versions.length > 1 ? this.versions[1].version : this.versions[0]?.version;
        await this.loadDiff();
      } catch (e) { toast(e.message, 'error'); }
    },

    async loadDiff() {
      if (!this.diffFrom) { this.diff = []; return; }
      const to = this.diffTo ? `&to=${this.diffTo}` : '';
      try { this.diff = await api('GET', `/api/prompts/${this.node.id}/diff?from=${this.diffFrom}${to}`); } catch (e) { toast(e.message, 'error'); }
    },

    compare(v) {
      this.diffFrom = v;
      this.diffTo = '';
      this.loadDiff();
    },

    get diffStats() {
      return { add: this.diff.filter((l) => l.tag === 'insert').length, del: this.diff.filter((l) => l.tag === 'delete').length };
    },

    /** Свернуть длинные неизменённые участки diff до 3 строк контекста. */
    diffRows() {
      const rows = [];
      const ctx = 3;
      const lines = this.diff;
      const changed = lines.map((l) => l.tag !== 'equal');
      for (let i = 0; i < lines.length; i++) {
        const near = changed.slice(Math.max(0, i - ctx), i + ctx + 1).some(Boolean);
        if (lines[i].tag !== 'equal' || near) rows.push({ ...lines[i], i });
        else if (rows.length === 0 || rows[rows.length - 1].gap === undefined) rows.push({ gap: true, i });
      }
      return rows;
    },

    async restoreVersion(v) {
      if (!confirm(`Восстановить текст версии ${v}? Он сохранится как новая версия.`)) return;
      try {
        await api('POST', `/api/prompts/${this.node.id}/restore/${v}`);
        await this.select(this.node.id, true);
        await this.loadVersions();
        toast(`Восстановлено из версии ${v}`);
      } catch (e) { toast(e.message, 'error'); }
    },

    async loadPosts() {
      if (!this.node?.key) { this.posts = []; this.postsByVersion = []; return; }
      try {
        const res = await api('GET', `/api/articles?prompt_key=${encodeURIComponent(this.node.key)}&per_page=100&sort=created`);
        this.posts = res.data;
        const groups = {};
        for (const p of res.data) (groups[p.prompt_version ?? 0] ||= []).push(p);
        this.postsByVersion = Object.keys(groups).map(Number).sort((a, b) => b - a).map((v) => {
          const list = groups[v];
          const accepted = list.filter((p) => p.reviewed_at && ['draft', 'scheduled', 'archived'].includes(p.state)).length;
          const rejected = list.filter((p) => p.state === 'rejected').length;
          return { version: v, posts: list, accepted, rejected, rate: accepted + rejected ? Math.round((accepted * 100) / (accepted + rejected)) : null };
        });
      } catch (e) { toast(e.message, 'error'); }
    },

    statusOf(p) { return displayStatus(p); },
    fmtDateTime,
    fmtRelative,

    openDialog(kind, parentId = null) {
      this.dialog = 'create';
      const parent = parentId ?? (this.node ? (this.isFolder ? this.node.id : this.node.parent_id) : null);
      this.dialogForm = { kind, key: '', title: '', parent_id: parent || '' };
      this.$nextTick(() => this.$refs.dialogFirst?.focus());
    },

    async createNode() {
      const f = this.dialogForm;
      try {
        const n = await api('POST', '/api/prompts', {
          kind: f.kind, key: f.kind === 'prompt' ? f.key.trim() : null, title: f.title.trim(), parent_id: f.parent_id || null,
          body: f.kind === 'prompt' ? '' : '', description: '',
        });
        this.dialog = null;
        if (f.parent_id) this.expanded[f.parent_id] = true;
        await this.loadTree();
        await this.select(n.id, true);
        if (n.kind === 'prompt') this.$nextTick(() => this.$refs.body?.focus());
        toast(n.kind === 'folder' ? 'Папка создана' : 'Промпт создан');
      } catch (e) { toast(e.message, 'error'); }
    },

    openMove() { this.dialog = 'move'; this.moveTarget = this.node?.parent_id || ''; },

    async moveTo(id, parentId) {
      try {
        await api('POST', `/api/prompts/${id}/move`, { parent_id: parentId || null, position: 9999 });
        if (parentId) this.expanded[parentId] = true;
        await this.loadTree();
        if (this.node?.id === id) this.node = await api('GET', `/api/prompts/${id}`);
        toast('Перемещено');
      } catch (e) { toast(e.message, 'error'); }
    },

    async confirmMove() {
      await this.moveTo(this.node.id, this.moveTarget);
      this.dialog = null;
    },

    dragStart(e, id) { this.dragId = id; e.dataTransfer.effectAllowed = 'move'; e.dataTransfer.setData('text/plain', id); },
    dragOver(e, row) {
      if (!this.dragId || (row && (row.kind !== 'folder' || row.id === this.dragId))) return;
      e.preventDefault();
      this.dropId = row ? row.id : 'root';
    },
    async drop(e, row) {
      e.preventDefault();
      const id = this.dragId;
      this.dragId = null;
      this.dropId = null;
      if (id) await this.moveTo(id, row ? row.id : null);
    },

    async remove() {
      const what = this.isFolder ? `папку «${this.node.title}» со всем содержимым` : `промпт «${this.node.key}» и всю его историю`;
      if (!confirm(`Удалить ${what}?`)) return;
      try {
        await api('DELETE', `/api/prompts/${this.node.id}`);
        this.node = null;
        this.original = '';
        await this.loadTree();
        toast('Удалено');
      } catch (e) { toast(e.message, 'error'); }
    },

    insertInclude(key) {
      const ta = this.$refs.body;
      const text = `{{> ${key}}}`;
      ta.focus();
      if (!document.execCommand('insertText', false, text)) ta.setRangeText(text, ta.selectionStart, ta.selectionEnd, 'end');
      this.form.body = ta.value;
    },

    selectByKey(key) {
      const n = this.flat().find((x) => (x.key || '').toLowerCase() === key.toLowerCase());
      if (n) this.select(n.id);
      else toast(`Промпт «${key}» не найден`, 'error');
    },

    async exportAll() {
      try {
        const files = await api('GET', '/api/prompts/export');
        const blob = new Blob([JSON.stringify(files, null, 2)], { type: 'application/json' });
        const a = document.createElement('a');
        a.href = URL.createObjectURL(blob);
        a.download = `prompts-${todayLocal()}.json`;
        a.click();
        URL.revokeObjectURL(a.href);
        toast(`Выгружено промптов: ${files.length}`);
      } catch (e) { toast(e.message, 'error'); }
    },

    async importFiles(e) {
      const files = [...(e.target.files || [])];
      e.target.value = '';
      if (!files.length) return;
      try {
        const contents = await Promise.all(files.map((f) => f.text()));
        const res = await api('POST', '/api/prompts/import', { files: contents });
        await this.loadTree();
        toast(`Импортировано: ${res.length}`);
        if (res[0]) await this.select(res[0].id, true);
      } catch (err) { toast(err.message, 'error'); }
    },

    onKey(e) {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') { e.preventDefault(); this.save(); }
      if (e.key === 'Escape' && this.dialog) this.dialog = null;
    },
  };
}

/** Настройки: площадки публикации. */
function settingsPage(initialTargets) {
  return {
    targets: initialTargets || [],
    form: { platform: 'telegram', external_id: '', display_name: '', parse_mode: 'html' },
    adding: false,
    editing: null,
    editName: '',
    async add() {
      try {
        const t = await api('POST', '/api/targets', {
          platform: this.form.platform,
          external_id: this.form.external_id.trim(),
          display_name: this.form.display_name.trim() || this.form.external_id.trim(),
          config: { parse_mode: this.form.parse_mode },
        });
        this.targets.unshift(t);
        this.form = { platform: 'telegram', external_id: '', display_name: '', parse_mode: 'html' };
        this.adding = false;
        toast('Площадка добавлена');
      } catch (e) { toast(e.message, 'error'); }
    },
    async toggle(t) {
      try {
        const updated = await api('PUT', `/api/targets/${t.id}`, { is_active: !t.is_active });
        Object.assign(t, updated);
        toast(updated.is_active ? 'Площадка включена' : 'Площадка выключена');
      } catch (e) { toast(e.message, 'error'); }
    },
    startEdit(t) { this.editing = t.id; this.editName = t.display_name; },
    async saveName(t) {
      try {
        Object.assign(t, await api('PUT', `/api/targets/${t.id}`, { display_name: this.editName.trim() }));
        this.editing = null;
      } catch (e) { toast(e.message, 'error'); }
    },
    async remove(t) {
      if (!confirm(`Удалить площадку «${t.display_name}»?`)) return;
      try {
        await api('DELETE', `/api/targets/${t.id}`);
        this.targets = this.targets.filter((x) => x.id !== t.id);
        toast('Площадка удалена');
      } catch (e) { toast(e.message, 'error'); }
    },
  };
}

document.addEventListener('alpine:init', () => {
  Alpine.data('postEditor', postEditor);
  Alpine.data('inboxPage', inboxPage);
  Alpine.data('calendarPage', calendarPage);
  Alpine.data('promptsPage', promptsPage);
  Alpine.data('settingsPage', settingsPage);
  Alpine.data('toasts', () => ({
    items: [],
    add({ message, kind }) {
      const id = Math.random().toString(36).slice(2);
      this.items.push({ id, message, kind });
      setTimeout(() => this.dismiss(id), kind === 'error' ? 7000 : 3500);
    },
    dismiss(id) { this.items = this.items.filter((t) => t.id !== id); },
  }));
  Alpine.data('themeToggle', () => ({
    dark: document.documentElement.dataset.theme === 'dark',
    toggle() { this.dark = !this.dark; setTheme(this.dark ? 'dark' : 'light'); },
  }));
});

// Глобальные горячие клавиши: N — новый пост (вне полей ввода).
document.addEventListener('keydown', (e) => {
  if (e.defaultPrevented || e.ctrlKey || e.metaKey || e.altKey) return;
  const tag = (e.target.tagName || '').toLowerCase();
  if (['input', 'textarea', 'select'].includes(tag) || e.target.isContentEditable) return;
  if (e.key === 'n' || e.key === 'т') { e.preventDefault(); newPost(); }
});
