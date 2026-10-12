// Visual shell interactions. Media and room state remain owned by Kotlin.
const roomFromUrl = new URLSearchParams(location.search).get('roomId');
if (roomFromUrl) document.getElementById('roomId').value = roomFromUrl;
document.getElementById('chat-form').addEventListener('submit', event => event.preventDefault());

const roomCodeButton = document.getElementById('copyRoomBtn');
const copyStatus = document.getElementById('copy-status');
let copyTimeout;
roomCodeButton.addEventListener('click', async () => {
  const code = document.getElementById('current-room-id').textContent.trim();
  if (!code) return;
  try {
    await navigator.clipboard.writeText(code);
    roomCodeButton.classList.add('copied');
    roomCodeButton.setAttribute('aria-label', 'Código copiado');
    copyStatus.textContent = 'Código copiado';
    clearTimeout(copyTimeout);
    copyTimeout = setTimeout(() => {
      roomCodeButton.classList.remove('copied');
      roomCodeButton.setAttribute('aria-label', 'Copiar código da sala');
      copyStatus.textContent = '';
    }, 1500);
  } catch {
    clearTimeout(copyTimeout);
    roomCodeButton.classList.remove('copied');
    copyStatus.textContent = 'Não foi possível copiar. Código: ' + code;
  }
});

const stage = document.getElementById('screens-stage');
const previewsButton = document.getElementById('toggle-previews');
previewsButton.addEventListener('click', () => {
  const collapsed = stage.classList.toggle('rail-collapsed');
  previewsButton.setAttribute('aria-pressed', String(collapsed));
  previewsButton.setAttribute('aria-label', collapsed ? 'Mostrar prévias' : 'Ocultar prévias');
  previewsButton.title = collapsed ? 'Mostrar prévias' : 'Ocultar prévias';
  previewsButton.querySelector('svg').innerHTML = collapsed
    ? '<path d="m6 15 6-6 6 6"/>'
    : '<path d="m6 9 6 6 6-6"/>';
});

const workspace = document.getElementById('workspace');
document.getElementById('toggleSidebarBtn').addEventListener('click', () => {
  workspace.classList.add('chat-collapsed');
  document.getElementById('restoreSidebarBtn').focus();
});
document.getElementById('restoreSidebarBtn').addEventListener('click', () => {
  workspace.classList.remove('chat-collapsed');
  document.getElementById('toggleSidebarBtn').focus();
});

document.querySelectorAll('.device-picker').forEach(picker => {
  picker.addEventListener('toggle', () => {
    if (picker.open) {
      document.querySelectorAll('.device-picker').forEach(other => { if (other !== picker) other.open = false; });
      const panel = picker.querySelector('.picker-panel');
      panel.style.transform = '';
      const rect = panel.getBoundingClientRect();
      const shift = Math.max(12 - rect.left, Math.min(0, innerWidth - 12 - rect.right));
      panel.style.transform = `translateX(${shift}px)`;
    }
  });
  picker.querySelector('select').addEventListener('change', () => { picker.open = false; });
});

const qualityModal = document.getElementById('quality-modal');
const permissionModal = document.getElementById('permission-recovery');
const dialogs = [permissionModal, qualityModal];
const activeDialog = () => dialogs.find(dialog => !dialog.classList.contains('hidden'));
const focusableItems = dialog => [...dialog.querySelectorAll('button, input, select, summary, [tabindex="0"]')]
  .filter(item => !item.disabled && item.getClientRects().length);
dialogs.forEach(dialog => {
  let wasOpen = false;
  let returnFocus = null;
  new MutationObserver(() => {
    const isOpen = !dialog.classList.contains('hidden');
    if (isOpen === wasOpen) return;
    wasOpen = isOpen;
    const active = activeDialog();
    document.querySelectorAll('#join-screen, #app-screen, #tray-panel').forEach(surface => { surface.inert = !!active; });
    qualityModal.inert = active === permissionModal;
    if (isOpen) {
      returnFocus = document.activeElement;
      if (active === dialog) focusableItems(dialog)[0]?.focus();
    } else {
      if (returnFocus?.isConnected && returnFocus.getClientRects().length && !returnFocus.closest('[inert]')) returnFocus.focus();
      else if (active) focusableItems(active)[0]?.focus();
      returnFocus = null;
    }
  }).observe(dialog, { attributes: true, attributeFilter: ['class'] });
});

const context = document.getElementById('participant-context');
let contextFocus = null;
function closeContext(restoreFocus = false) {
  if (context.classList.contains('hidden')) return;
  context.classList.add('hidden');
  context.replaceChildren();
  if (restoreFocus) contextFocus?.focus();
  contextFocus = null;
}
function openContext(tile, x, y) {
  const owner = tile.dataset.ownerId;
  const name = tile.dataset.username || 'Participante';
  const video = tile.classList.contains('screen-tile');
  const self = owner === 'self';
  contextFocus = document.activeElement;
  context.innerHTML = '<h3></h3>'
    + (self ? '<div class="context-note">Use o microfone no dock para alterar seu áudio.</div>'
      : '<label for="context-volume">Volume para você</label><input id="context-volume" type="range" min="0" max="100"><button type="button" data-action="mute"></button>')
    + '<button type="button" data-action="video" class="hidden"></button><div class="context-note">Estas opções afetam apenas sua visualização.</div>';
  context.querySelector('h3').textContent = name;
  const slider = context.querySelector('input');
  if (slider) {
    const screen = video && !tile.classList.contains('camera-tile');
    const source = screen ? tile.querySelector('input[type=range]') : document.getElementById('user-list-' + owner)?.querySelector('input[type=range]');
    const audio = screen ? tile.querySelector('video') : document.getElementById(`remote-audio-${owner}`);
    slider.value = source?.value || Math.round((audio?.volume ?? 1) * 100);
    slider.addEventListener('input', () => {
      if (source) {
        source.value = slider.value;
        source.dispatchEvent(new Event('input', { bubbles: true }));
      } else if (audio) audio.volume = Number(slider.value) / 100;
    });
    const mute = context.querySelector('[data-action=mute]');
    const refreshMute = () => { mute.textContent = Number(slider.value) === 0 ? 'Reativar áudio para você' : 'Silenciar áudio para você'; };
    slider.addEventListener('input', refreshMute);
    refreshMute();
    mute.addEventListener('click', () => {
      if (Number(slider.value) === 0) slider.value = slider.dataset.previous || '100';
      else { slider.dataset.previous = slider.value; slider.value = '0'; }
      slider.dispatchEvent(new Event('input', { bubbles: true }));
      refreshMute();
    });
  }
  const hideVideo = context.querySelector('[data-action=video]');
  if (video) {
    hideVideo.classList.remove('hidden');
    const refreshVideo = () => { hideVideo.textContent = tile.classList.contains('video-hidden') ? 'Mostrar vídeo para você' : 'Ocultar vídeo para você'; };
    refreshVideo();
    hideVideo.addEventListener('click', () => { tile.classList.toggle('video-hidden'); refreshVideo(); });
  }
  context.classList.remove('hidden');
  context.style.left = Math.max(8, Math.min(x, innerWidth - context.offsetWidth - 8)) + 'px';
  context.style.top = Math.max(8, Math.min(y, innerHeight - context.offsetHeight - 8)) + 'px';
  (slider || context.querySelector('button:not(.hidden)'))?.focus();
}
document.addEventListener('contextmenu', event => {
  const tile = event.target.closest('.participant-tile, .screen-tile');
  if (!tile) return;
  event.preventDefault();
  openContext(tile, event.clientX, event.clientY);
});
document.addEventListener('click', event => {
  const options = event.target.closest('[data-participant-options]');
  if (options) {
    const tile = options.closest('.participant-tile, .screen-tile');
    const rect = options.getBoundingClientRect();
    openContext(tile, rect.left, rect.bottom + 6);
    return;
  }
  if (!event.target.closest('#participant-context')) closeContext();
  if (!event.target.closest('.device-picker')) document.querySelectorAll('.device-picker').forEach(picker => { picker.open = false; });
});
document.addEventListener('keydown', event => {
  const dialog = activeDialog();
  if (event.key === 'Tab' && dialog) {
    const items = focusableItems(dialog);
    const first = items[0], last = items.at(-1);
    if (first && (!items.includes(document.activeElement) || event.shiftKey && document.activeElement === first || !event.shiftKey && document.activeElement === last)) {
      event.preventDefault();
      (event.shiftKey ? last : first).focus();
    }
    return;
  }
  if (event.key === 'Escape') {
    if (dialog) {
      event.preventDefault();
      document.getElementById(dialog === permissionModal ? 'close-permission-help' : 'cancel-share').click();
      return;
    }
    const contextOpen = !context.classList.contains('hidden');
    closeContext(true);
    document.querySelectorAll('.device-picker').forEach(picker => { picker.open = false; });
    // Esc closes the tray panel; media keeps running in the hidden window.
    if (!contextOpen && document.body.classList.contains('compact-tray')) globalThis.__screenshareDesktopBridge?.hideMainWindow();
    return;
  }
  const tile = document.activeElement?.closest('.participant-tile, .screen-tile');
  if ((event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10'))
    && tile) {
    event.preventDefault();
    const rect = tile.getBoundingClientRect();
    openContext(tile, rect.left, rect.bottom + 6);
  }
});
