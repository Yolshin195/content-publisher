document.addEventListener('DOMContentLoaded', () => {
  const form = document.getElementById('article-form');
  if (form) {
    form.addEventListener('submit', onSubmitArticleForm);
  }

  const addVideoBtn = document.getElementById('add-video-link');
  if (addVideoBtn) {
    addVideoBtn.addEventListener('click', () => addVideoLinkRow());
  }

  const coverInput = document.getElementById('cover-upload');
  if (coverInput) {
    coverInput.addEventListener('change', onCoverUpload);
  }
});

function addVideoLinkRow() {
  const container = document.getElementById('video-links');
  const addBtn = document.getElementById('add-video-link');
  const row = document.createElement('div');
  row.className = 'video-link-row';
  row.innerHTML = `
    <select name="video_platform">
      <option value="youtube">YouTube</option>
      <option value="vk_video">VK Video</option>
      <option value="rutube">Rutube</option>
      <option value="vimeo">Vimeo</option>
      <option value="other">Другое</option>
    </select>
    <input type="url" name="video_url" placeholder="https://...">
  `;
  container.insertBefore(row, addBtn);
}

async function onCoverUpload(event) {
  const file = event.target.files[0];
  if (!file) return;
  try {
    const resp = await fetch('/api/media/upload-url', { method: 'POST' });
    if (!resp.ok) throw new Error('media service unavailable');
    const data = await resp.json();
    await fetch(data.upload_url, { method: 'PUT', body: file });
    document.getElementById('cover-media-id').value = data.media_id;
  } catch (e) {
    alert('Не удалось загрузить картинку: ' + e.message);
  }
}

function slugify(title) {
  return title.toLowerCase()
    .replace(/[^a-z0-9а-яё\s-]/gi, '')
    .trim()
    .replace(/\s+/g, '-')
    .slice(0, 80) + '-' + Date.now().toString(36);
}

async function onSubmitArticleForm(event) {
  event.preventDefault();
  const form = event.target;
  const isEdit = form.dataset.isEdit === 'true';
  const articleId = form.dataset.articleId;

  const payload = {
    title: form.title.value,
    content_html: form.content_html.value,
    excerpt: form.excerpt.value || null,
    cover_media_id: document.getElementById('cover-media-id').value || null,
  };

  let article;
  if (isEdit) {
    const resp = await fetch(`/api/articles/${articleId}`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
    article = await resp.json();
  } else {
    payload.slug = slugify(payload.title);
    const resp = await fetch('/api/articles', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
    article = await resp.json();
  }

  const videoRows = form.querySelectorAll('.video-link-row');
  for (const row of videoRows) {
    const platform = row.querySelector('[name=video_platform]').value;
    const url = row.querySelector('[name=video_url]').value;
    if (url) {
      await fetch(`/api/articles/${article.id}/video-links`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ platform, url, is_primary: false }),
      });
    }
  }

  const scheduledAt = form.scheduled_at.value;
  const targetIds = Array.from(form.querySelectorAll('[name=target_ids]:checked')).map((el) => el.value);
  if (scheduledAt && targetIds.length > 0) {
    await fetch(`/api/articles/${article.id}/schedule`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ scheduled_at: new Date(scheduledAt).toISOString(), target_ids: targetIds }),
    });
  }

  window.location.href = `/articles/${article.id}`;
}
