/* Shared window chrome: titlebar + compact sidebar. Requires icons.js */
function sidebar(active, themeOn) {
  const item = (ic, label, on) =>
    `<div class="nav-item ${on?'active':''}">${ICON[ic](18)}<span>${label}</span></div>`;
  const seg = (t) => `<span class="${t===themeOn?'on':''}">${t==='sys'?'시스템':t==='light'?'라이트':'다크'}</span>`;
  return `<aside class="sidebar">
    <div class="side-brand"><span class="mark">${ICON.mark(17)}</span><span class="name">LogiFlow</span></div>
    <nav class="nav">
      ${item('swap','전환 관리', active==='switch')}
      ${item('gear','설정', active==='settings')}
      ${item('diag','진단', active==='diag')}
    </nav>
    <div class="side-spacer"></div>
    <div class="side-status"><span class="mark" style="color:var(--muted)">${ICON.plug(16)}</span>
      <span class="dot"></span><span>LogiFlow 실행 중</span></div>
  </aside>`;
}
function titlebar() {
  return `<div class="titlebar">
    <div class="title-left"><span class="mark">${ICON.mark(18)}</span><span class="name">LogiFlow</span></div>
    <div class="win-controls">
      <div class="win-btn">${ICON.wmin(11)}</div>
      <div class="win-btn">${ICON.wmax(11)}</div>
      <div class="win-btn close">${ICON.wclose(11)}</div>
    </div>
  </div>`;
}
function windowShell(active, themeOn, contentHTML) {
  return `<section class="window">
    ${titlebar()}
    <div class="app">${sidebar(active, themeOn)}
      <div class="content">${contentHTML}</div>
    </div>
  </section>`;
}
function mountPanes(active, buildContent) {
  document.getElementById('stage').innerHTML =
    `<div class="pane"><div class="theme-light">${windowShell(active,'light',buildContent())}</div>
       <div class="pane-cap">라이트</div></div>
     <div class="pane d"><div class="theme-dark">${windowShell(active,'dark',buildContent())}</div>
       <div class="pane-cap">다크</div></div>`;
}
