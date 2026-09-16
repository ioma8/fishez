const status = document.querySelector('#copy-status');
document.querySelectorAll('[data-copy]').forEach((button) => button.addEventListener('click', async () => {
  try { await navigator.clipboard.writeText(button.closest('.command').querySelector('code').textContent); button.textContent = 'Copied'; status.textContent = 'Command copied to clipboard.'; setTimeout(() => { button.textContent = 'Copy'; }, 1600); }
  catch { button.textContent = 'Select text'; status.textContent = 'Select and copy the command.'; }
}));
const image = document.querySelector('#demo-image');
const play = document.querySelector('#play-demo');
play.addEventListener('click', () => {
  const playing = play.closest('.demo-stage').classList.toggle('playing');
  image.src = playing ? './assets/demo.gif' : './assets/demo-poster.png';
  play.textContent = playing ? 'Stop demo' : 'Play demo';
});
