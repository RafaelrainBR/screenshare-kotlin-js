// Visual shell interactions. Media and room state remain owned by Kotlin.
const roomFromUrl = new URLSearchParams(location.search).get('roomId');
if (roomFromUrl) document.getElementById('roomId').value = roomFromUrl;
document.getElementById('join-form').addEventListener('submit', event => event.preventDefault());
document.getElementById('chat-form').addEventListener('submit', event => event.preventDefault());

const roomCodeButton = document.getElementById('copyRoomBtn');
roomCodeButton.addEventListener('click', async () => {
  const code = document.getElementById('current-room-id').textContent.trim();
  if (!code) return;
  try {
    await navigator.clipboard.writeText(code);
    roomCodeButton.classList.add('copied');
    roomCodeButton.setAttribute('aria-label', 'Código copiado');
    setTimeout(() => {
      roomCodeButton.classList.remove('copied');
      roomCodeButton.setAttribute('aria-label', 'Copiar código da sala');
    }, 1500);
  } catch {
    roomCodeButton.setAttribute('aria-label', 'Não foi possível copiar o código');
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
document.getElementById('toggleSidebarBtn').addEventListener('click', () => workspace.classList.add('chat-collapsed'));
document.getElementById('restoreSidebarBtn').addEventListener('click', () => workspace.classList.remove('chat-collapsed'));

document.querySelectorAll('.device-picker').forEach(picker => {
  picker.addEventListener('toggle', () => {
    if (picker.open) document.querySelectorAll('.device-picker').forEach(other => { if (other !== picker) other.open = false; });
  });
  picker.querySelector('select').addEventListener('change', () => { picker.open = false; });
});

const qualityModal = document.getElementById('quality-modal');
let modalReturnFocus = null;
new MutationObserver(() => {
  if (!qualityModal.classList.contains('hidden')) {
    modalReturnFocus = document.activeElement;
    const panel = qualityModal.querySelector('.modal-panel');
    panel.tabIndex = -1;
    panel.focus();
  } else {
    modalReturnFocus?.focus();
    modalReturnFocus = null;
  }
}).observe(qualityModal, { attributes: true, attributeFilter: ['class'] });

const context = document.getElementById('participant-context');
let contextTile = null;
let contextFocus = null;
function closeContext() {
  if (context.classList.contains('hidden')) return;
  context.classList.add('hidden');
  context.replaceChildren();
  contextTile = null;
  contextFocus?.focus();
  contextFocus = null;
}
function openContext(tile, x, y) {
  const owner = tile.dataset.ownerId;
  const name = tile.dataset.username || 'Participante';
  const video = tile.classList.contains('screen-tile');
  const self = owner === 'self';
  contextTile = tile;
  contextFocus = document.activeElement;
  context.innerHTML = '<h3></h3>'
    + (self ? '<div class="context-note">Use o microfone no dock para alterar seu áudio.</div>'
      : '<label for="context-volume">Volume para você</label><input id="context-volume" type="range" min="0" max="100"><button type="button" data-action="mute"></button>')
    + '<button type="button" data-action="video" class="hidden"></button><div class="context-note">Estas opções afetam apenas sua visualização.</div>';
  context.querySelector('h3').textContent = name;
  const slider = context.querySelector('input');
  if (slider) {
    const source = document.getElementById('user-list-' + owner)?.querySelector('input[type=range]');
    const audio = document.getElementById(`remote-audio-${owner}`);
    slider.value = source?.value || Math.round((audio?.volume ?? 1) * 100);
    slider.addEventListener('input', () => {
      if (source) {
        source.value = slider.value;
        source.dispatchEvent(new Event('input', { bubbles: true }));
      } else if (audio) audio.volume = Number(slider.value) / 100;
    });
    const mute = context.querySelector('[data-action=mute]');
    const refreshMute = () => { mute.textContent = Number(slider.value) === 0 ? 'Reativar áudio para você' : 'Silenciar áudio para você'; };
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
  const avatar = event.target.closest('.participant-tile');
  if (avatar) {
    const rect = avatar.getBoundingClientRect();
    openContext(avatar, rect.left, rect.bottom + 6);
    return;
  }
  const videoTile = event.target.closest('.screen-tile');
  if (videoTile && !event.target.closest('button, input')) {
    const rect = videoTile.getBoundingClientRect();
    openContext(videoTile, rect.left, rect.bottom + 6);
    return;
  }
  if (!event.target.closest('#participant-context')) closeContext();
  if (!event.target.closest('.device-picker')) document.querySelectorAll('.device-picker').forEach(picker => { picker.open = false; });
});
document.addEventListener('keydown', event => {
  if (event.key === 'Tab' && !qualityModal.classList.contains('hidden')) {
    const items = [...qualityModal.querySelectorAll('button, input, select')].filter(item => !item.disabled && item.offsetParent !== null);
    const first = items[0], last = items.at(-1);
    if (first && (event.shiftKey && document.activeElement === first || !event.shiftKey && document.activeElement === last)) {
      event.preventDefault();
      (event.shiftKey ? last : first).focus();
    }
    return;
  }
  if (event.key === 'Escape') {
    if (!document.getElementById('quality-modal').classList.contains('hidden')) {
      document.getElementById('cancel-share').click();
      return;
    }
    closeContext();
    document.querySelectorAll('.device-picker').forEach(picker => { picker.open = false; });
    return;
  }
  const tile = document.activeElement;
  if ((event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10'))
    && tile?.matches('.participant-tile, .screen-tile')) {
    event.preventDefault();
    const rect = tile.getBoundingClientRect();
    openContext(tile, rect.left, rect.bottom + 6);
  }
});
