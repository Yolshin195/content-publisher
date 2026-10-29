#!/usr/bin/env bash
# Синхронизация справочника промптов с папкой prompts/ в git — чтобы видеть
# изменения промптов через `git diff` и хранить их историю рядом с кодом.
#
#   CP_URL=https://publisher.example.com CP_TOKEN=... ./scripts/prompts-sync.sh pull
#   CP_URL=https://publisher.example.com CP_TOKEN=... ./scripts/prompts-sync.sh push
#
# pull — выгружает все промпты из сервиса в prompts/<папки>/<key>.md (файлы удалённых
#        промптов тоже удаляются), дальше обычные git diff / git commit.
# push — загружает prompts/**/*.md обратно; изменившиеся промпты получают новую версию
#        в сервисе (видно во вкладке «История»), новые — создаются.
#
# Формат файла — Markdown с front matter (key, title, description, path), тот же,
# что у кнопок «Скачать .md» / «Импорт .md» в интерфейсе.
set -euo pipefail

CP_URL="${CP_URL:-http://localhost:8080}"
CP_TOKEN="${CP_TOKEN:-${API_TOKEN:-}}"
DIR="${PROMPTS_DIR:-prompts}"
cmd="${1:-}"

auth=()
if [ -n "$CP_TOKEN" ]; then auth=(-H "Authorization: Bearer $CP_TOKEN"); fi

case "$cmd" in
  pull)
    mkdir -p "$DIR"
    curl -fsS "${auth[@]}" "$CP_URL/api/prompts/export" | python3 -c '
import json, os, sys
target = sys.argv[1]
files = json.load(sys.stdin)
keep = set()
for f in files:
    rel = f["path"]
    path = os.path.normpath(os.path.join(target, rel))
    if not path.startswith(os.path.normpath(target) + os.sep):
        sys.exit("unsafe path: " + rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as fh:
        fh.write(f["content"])
    keep.add(path)
removed = 0
for root, _, names in os.walk(target):
    for n in names:
        p = os.path.normpath(os.path.join(root, n))
        if n.endswith(".md") and p not in keep:
            os.remove(p)
            removed += 1
print(f"→ выгружено промптов: {len(files)}, удалено устаревших файлов: {removed}")
' "$DIR"
    git status --short -- "$DIR" || true
    ;;
  push)
    [ -d "$DIR" ] || { echo "нет каталога $DIR — сначала выполните pull"; exit 1; }
    python3 - "$DIR" > /tmp/prompts-import.json <<'PY'
import json, os, sys
files = []
for root, _, names in os.walk(sys.argv[1]):
    for n in sorted(names):
        if n.endswith(".md"):
            with open(os.path.join(root, n), encoding="utf-8") as fh:
                files.append(fh.read())
json.dump({"files": files}, sys.stdout, ensure_ascii=False)
PY
    curl -fsS "${auth[@]}" -H "Content-Type: application/json" -X POST \
      --data-binary @/tmp/prompts-import.json "$CP_URL/api/prompts/import" | python3 -c '
import json, sys
nodes = json.load(sys.stdin)
for n in nodes:
    print("  %s: v%s" % (n["key"], n["version"]))
print(f"→ загружено: {len(nodes)}")
'
    rm -f /tmp/prompts-import.json
    ;;
  *)
    echo "usage: $0 pull|push   (env: CP_URL, CP_TOKEN)"; exit 1 ;;
esac
