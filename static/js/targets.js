/**
 * JavaScript для управления площадками публикации (targets)
 */

(function() {
    'use strict';

    // Инициализация при загрузке страницы
    document.addEventListener('DOMContentLoaded', function() {
        initTargetList();
        initTargetForm();
    });

    /**
     * Инициализация страницы списка площадок
     */
    function initTargetList() {
        const deleteButtons = document.querySelectorAll('[data-target-id]');
        const modal = document.getElementById('delete-modal');
        const cancelBtn = document.getElementById('cancel-delete');
        const confirmBtn = document.getElementById('confirm-delete');
        const targetNameEl = document.getElementById('delete-target-name');

        if (!modal || deleteButtons.length === 0) return;

        let targetIdToDelete = null;

        // Обработчик кнопок удаления
        deleteButtons.forEach(function(btn) {
            btn.addEventListener('click', function() {
                targetIdToDelete = this.getAttribute('data-target-id');
                const targetName = this.getAttribute('data-target-name');
                targetNameEl.textContent = targetName;
                modal.style.display = 'block';
            });
        });

        // Отмена удаления
        if (cancelBtn) {
            cancelBtn.addEventListener('click', function() {
                modal.style.display = 'none';
                targetIdToDelete = null;
            });
        }

        // Подтверждение удаления
        if (confirmBtn) {
            confirmBtn.addEventListener('click', function() {
                if (!targetIdToDelete) return;

                fetch('/api/targets/' + targetIdToDelete, {
                    method: 'DELETE',
                    headers: {
                        'Accept': 'application/json'
                    }
                })
                .then(function(response) {
                    if (response.ok) {
                        window.location.href = '/targets';
                    } else {
                        alert('Ошибка при удалении площадки');
                        modal.style.display = 'none';
                    }
                })
                .catch(function(error) {
                    console.error('Error:', error);
                    alert('Ошибка при удалении площадки');
                    modal.style.display = 'none';
                });
            });
        }

        // Закрытие модального окна по клику на backdrop
        const backdrop = modal.querySelector('.modal-backdrop');
        if (backdrop) {
            backdrop.addEventListener('click', function() {
                modal.style.display = 'none';
                targetIdToDelete = null;
            });
        }
    }

    /**
     * Инициализация формы создания/редактирования площадки
     */
    function initTargetForm() {
        const form = document.getElementById('target-form');
        if (!form) return;

        const configTextarea = document.getElementById('config_json');
        const jsonError = document.getElementById('json-error');

        // Валидация JSON при вводе
        if (configTextarea) {
            configTextarea.addEventListener('input', function() {
                const value = this.value.trim();
                if (!value) {
                    hideJsonError();
                    return;
                }
                try {
                    JSON.parse(value);
                    hideJsonError();
                } catch (e) {
                    // Не показываем ошибку пока пользователь печатает
                    // Проверим только при отправке формы
                }
            });

            configTextarea.addEventListener('blur', function() {
                const value = this.value.trim();
                if (!value) {
                    hideJsonError();
                    return;
                }
                try {
                    JSON.parse(value);
                    hideJsonError();
                } catch (e) {
                    showJsonError();
                }
            });
        }

        // Обработка отправки формы
        form.addEventListener('submit', function(e) {
            e.preventDefault();

            // Проверяем JSON
            const configValue = configTextarea ? configTextarea.value.trim() : '{}';
            if (configValue && !isValidJson(configValue)) {
                showJsonError();
                configTextarea.focus();
                return;
            }

            const isEdit = form.getAttribute('data-is-edit') === 'true';
            const targetId = form.getAttribute('data-target-id');
            const formData = new FormData(form);

            const data = {
                display_name: formData.get('display_name'),
                external_id: formData.get('external_id'),
                is_active: formData.get('is_active') === 'on',
                config: configValue ? JSON.parse(configValue) : {}
            };

            // Для создания добавляем платформу
            if (!isEdit) {
                data.platform = formData.get('platform');
            }

            const url = isEdit
                ? '/api/targets/' + targetId
                : '/api/targets';

            const method = isEdit ? 'PUT' : 'POST';

            fetch(url, {
                method: method,
                headers: {
                    'Content-Type': 'application/json',
                    'Accept': 'application/json'
                },
                body: JSON.stringify(data)
            })
            .then(function(response) {
                if (response.ok) {
                    window.location.href = '/targets';
                } else {
                    return response.json().then(function(err) {
                        throw new Error(err.message || 'Ошибка при сохранении');
                    }).catch(function() {
                        throw new Error('Ошибка при сохранении');
                    });
                }
            })
            .catch(function(error) {
                console.error('Error:', error);
                alert(error.message || 'Ошибка при сохранении площадки');
            });
        });
    }

    /**
     * Проверка валидности JSON
     */
    function isValidJson(str) {
        try {
            JSON.parse(str);
            return true;
        } catch (e) {
            return false;
        }
    }

    /**
     * Показать ошибку JSON
     */
    function showJsonError() {
        const errorEl = document.getElementById('json-error');
        const textarea = document.getElementById('config_json');
        if (errorEl) errorEl.style.display = 'block';
        if (textarea) textarea.style.borderColor = '#d92d20';
    }

    /**
     * Скрыть ошибку JSON
     */
    function hideJsonError() {
        const errorEl = document.getElementById('json-error');
        const textarea = document.getElementById('config_json');
        if (errorEl) errorEl.style.display = 'none';
        if (textarea) textarea.style.borderColor = '';
    }

})();
