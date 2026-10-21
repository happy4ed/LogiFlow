using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.Threading;
using System.Windows.Forms;
using LogiFlow.Config;
using LogiFlow.Diagnostics;
using LogiFlow.Ui;

namespace LogiFlow
{
    /// <summary>
    /// 승인 시안(사이드바 내비 + 전환 관리·설정·진단 3화면)의 WinForms 구현.
    /// 창 크기는 페이지와 무관하게 고정(리사이즈·최소크기 허용), 내용이 많으면 스크롤.
    /// </summary>
    internal sealed class MainForm : Form
    {
        private const int ContentW = 700;

        private readonly TrayAppContext _tray;
        private AppConfig Config { get { return _tray.Config; } }
        private DiagLogger Log { get { return _tray.Log; } }

        private Panel _sidebar;
        private Panel _pageHost;
        private NavButton _navSwitch, _navSettings, _navDiag;
        private int _pageIndex;
        private bool _busy;

        public MainForm(TrayAppContext tray)
        {
            _tray = tray;
            BuildUi();
            Theme.Changed += OnThemeChanged;
        }

        private void BuildUi()
        {
            Text = "LogiFlow";
            Icon = AppIcon.Get();
            StartPosition = FormStartPosition.CenterScreen;
            ClientSize = new Size(900, 600);
            MinimumSize = new Size(780, 560);
            Font = new Font("Segoe UI", 9f);

            _sidebar = new Panel { Dock = DockStyle.Left, Width = 182, Padding = new Padding(12, 14, 12, 12) };
            BuildSidebar();

            _pageHost = new Panel { Dock = DockStyle.Fill, AutoScroll = true };

            Controls.Add(_pageHost);
            Controls.Add(_sidebar);

            ApplyChrome();
            FormClosing += OnFormClosing;
            ShowPage(0);
        }

        private void BuildSidebar()
        {
            DisposeChildren(_sidebar);
            Color sb = Theme.Sidebar;

            var brand = new Panel { Dock = DockStyle.Top, Height = 40, Padding = new Padding(4, 2, 4, 2), BackColor = sb };
            var mark = new IconView(Icons.Mark, 18) { Left = 4, Top = 9, Tint = Theme.Accent };
            var name = new Label { Text = "LogiFlow", Left = 30, Top = 8, AutoSize = true, Font = new Font("Segoe UI", 10f, FontStyle.Bold), ForeColor = Theme.Ink };
            brand.Controls.Add(mark);
            brand.Controls.Add(name);

            _navSwitch = MakeNav("전환 관리", Icons.Swap, 0);
            _navSettings = MakeNav("설정", Icons.Gear, 1);
            _navDiag = MakeNav("진단", Icons.Diag, 2);

            var navHost = new Panel { Dock = DockStyle.Top, Height = 3 * 36 + 2 * 4 + 8, Padding = new Padding(0, 8, 0, 0), BackColor = sb };
            _navDiag.Dock = DockStyle.Top; _navDiag.Height = 36;
            _navSettings.Dock = DockStyle.Top; _navSettings.Height = 36;
            _navSwitch.Dock = DockStyle.Top; _navSwitch.Height = 36;
            navHost.Controls.Add(_navDiag);
            navHost.Controls.Add(MakeGap(4));
            navHost.Controls.Add(_navSettings);
            navHost.Controls.Add(MakeGap(4));
            navHost.Controls.Add(_navSwitch);

            var status = new Panel { Dock = DockStyle.Bottom, Height = 32, BackColor = sb };
            var plug = new IconView(Icons.Plug, 16) { Left = 6, Top = 8, Tint = Theme.Muted };
            var dot = new IconView((g, r, c) => { using (var b = new SolidBrush(Theme.Ok)) g.FillEllipse(b, r.X, r.Y + r.Height / 2 - 3.5f, 7, 7); }, 16) { Left = 26, Top = 8 };
            var slabel = new Label { Text = "LogiFlow 실행 중", Left = 42, Top = 7, AutoSize = true, ForeColor = Theme.Muted, Font = new Font("Segoe UI", 8.5f) };
            status.Controls.Add(plug);
            status.Controls.Add(dot);
            status.Controls.Add(slabel);

            _sidebar.Controls.Add(navHost);
            _sidebar.Controls.Add(status);
            _sidebar.Controls.Add(brand);

            UpdateNavActive();
        }

        private NavButton MakeNav(string text, IconPaint icon, int index)
        {
            var nav = new NavButton(text, icon) { Font = new Font("Segoe UI", 9.5f) };
            nav.Selected += (s, e) => ShowPage(index);
            return nav;
        }

        private static Panel MakeGap(int h)
        {
            return new Panel { Dock = DockStyle.Top, Height = h, BackColor = Theme.Sidebar };
        }

        private void UpdateNavActive()
        {
            if (_navSwitch == null) return;
            _navSwitch.Active = _pageIndex == 0;
            _navSettings.Active = _pageIndex == 1;
            _navDiag.Active = _pageIndex == 2;
        }

        private void ApplyChrome()
        {
            BackColor = Theme.Content;
            if (_sidebar != null) _sidebar.BackColor = Theme.Sidebar;
            if (_pageHost != null) _pageHost.BackColor = Theme.Content;
        }

        private void OnThemeChanged(object s, EventArgs e)
        {
            if (IsDisposed) return;
            ApplyChrome();
            BuildSidebar();
            ShowPage(_pageIndex);
        }

        private static void DisposeChildren(Control host)
        {
            var old = new List<Control>();
            foreach (Control c in host.Controls) old.Add(c);
            host.Controls.Clear();
            foreach (Control c in old) c.Dispose();
        }

        public void ShowPage(int index)
        {
            _pageIndex = index;
            UpdateNavActive();
            DisposeChildren(_pageHost);
            Panel page;
            switch (index)
            {
                case 1: page = BuildSettingsPage(); break;
                case 2: page = BuildDiagPage(); break;
                default: page = BuildSwitchPage(); break;
            }
            page.Location = new Point(0, 0);
            _pageHost.Controls.Add(page);
        }

        // ---- 공통 빌더 ----
        private Label Title(string text)
        {
            return new Label { Text = text, AutoSize = true, Font = new Font("Segoe UI", 14f, FontStyle.Bold), ForeColor = Theme.Ink };
        }

        private Label Sub(string text)
        {
            return new Label { Text = text, AutoSize = true, Font = new Font("Segoe UI", 9f), ForeColor = Theme.Muted };
        }

        private Label SectionLabel(string text, string small)
        {
            var l = new Label { AutoSize = true, Font = new Font("Segoe UI", 9.5f, FontStyle.Bold), ForeColor = Theme.Ink };
            l.Text = small != null ? text + "   " + small : text;
            return l;
        }

        private Label GroupHead(string text)
        {
            return new Label { Text = text.ToUpper(), AutoSize = true, Font = new Font("Segoe UI", 8f, FontStyle.Bold), ForeColor = Theme.Muted };
        }

        private Panel Card(int x, int y, int w, int h)
        {
            var p = new ThemedCard { Left = x, Top = y, Width = w, Height = h };
            return p;
        }

        // ---- 전환 관리 ----
        private Panel BuildSwitchPage()
        {
            var page = new Panel { Width = ContentW + 48, Padding = new Padding(24, 20, 24, 20), BackColor = Theme.Content };
            int W = ContentW;
            int y = 20;

            var title = Title("전환 관리"); title.Location = new Point(24, y); page.Controls.Add(title);
            var sub = Sub("화면 끝으로 밀면 자동 전환 · 떨어뜨리면 수동"); sub.Location = new Point(24, y + 28); page.Controls.Add(sub);
            var refresh = new ThemedButton { Variant = ButtonVariant.Icon, Icon = Icons.Refresh, Width = 32, Height = 30, Left = 24 + W - 32, Top = y }; refresh.Click += (s, e) => _tray.Rescan();
            page.Controls.Add(refresh);
            y += 58;

            var canvas = new MonitorLayoutPanel { Left = 24, Top = y, Width = W, Height = 240 };
            int maxHost = _tray.MaxHostCount();
            int current = _tray.CurrentChannel();
            var channels = new List<int>();
            for (int h = 0; h < maxHost; h++) if (h != current) channels.Add(h);
            canvas.SetData(channels, current, Config.Attachments, Config.AliasFor);
            canvas.AttachmentsChanged += (s, e) =>
            {
                Config.Attachments.Clear();
                Config.Attachments.AddRange(canvas.GetAttachments());
                _tray.ApplyConfig();
            };
            page.Controls.Add(canvas);
            y += 240 + 14;

            // 채널 별칭
            var aliasLbl = SectionLabel("채널 별칭", "엣지에 둘 다른 PC 를 알아보기 쉽게"); aliasLbl.Location = new Point(24, y); page.Controls.Add(aliasLbl);
            y += 24;
            int ax = 24;
            int aliasCount = Math.Max(maxHost, 1);
            for (int ch = 0; ch < aliasCount; ch++)
            {
                int channel = ch;
                var chip = BuildAliasChip(channel);
                chip.Location = new Point(ax, y);
                page.Controls.Add(chip);
                ax += chip.Width + 8;
            }
            y += 42;

            // 등록된 기기
            var devices = _tray.Switchable();
            var devLbl = SectionLabel("등록된 기기", devices.Count + "개"); devLbl.Location = new Point(24, y); page.Controls.Add(devLbl);
            y += 26;
            if (devices.Count == 0)
            {
                var none = new Label { Text = "전환 가능한 기기가 없습니다. [진단]에서 새로 고침 하세요.", AutoSize = true, ForeColor = Theme.Muted, Location = new Point(24, y) };
                page.Controls.Add(none);
                y += 28;
            }
            foreach (var d in devices)
            {
                var row = BuildDeviceRow(d, W);
                row.Location = new Point(24, y);
                page.Controls.Add(row);
                y += row.Height;
            }
            y += 8;

            // footer
            var foot = BuildSwitchFooter(W, canvas);
            foot.Location = new Point(24, y);
            page.Controls.Add(foot);
            y += foot.Height + 20;

            page.Height = y;
            return page;
        }

        private Panel BuildAliasChip(int channel)
        {
            var chip = new ThemedCard { Height = 30, Padding = new Padding(8, 0, 8, 0) };
            var cn = new Label { Text = "채널 " + (channel + 1), AutoSize = true, Font = new Font("Segoe UI", 8f, FontStyle.Bold), ForeColor = Theme.Accent, Top = 7, Left = 8 };
            var al = new Label { Text = Config.AliasFor(channel), AutoSize = true, Font = new Font("Segoe UI", 8.5f), ForeColor = Theme.Ink, Top = 7 };
            al.Left = cn.Right + 8;
            var pen = new ThemedButton { Variant = ButtonVariant.Icon, Icon = Icons.Pencil, Width = 24, Height = 22, Top = 4 };
            pen.Left = al.Right + 6;
            pen.Click += (s, e) => EditAlias(channel);
            chip.Controls.Add(cn); chip.Controls.Add(al); chip.Controls.Add(pen);
            chip.Width = pen.Right + 8;
            // 칩 클릭도 편집.
            al.Click += (s, e) => EditAlias(channel);
            return chip;
        }

        private void EditAlias(int channel)
        {
            string cur = Config.AliasFor(channel);
            string v = Prompt.Show(this, "채널 별칭", string.Format("채널 {0} 의 별칭", channel + 1), cur);
            if (v == null) return;
            if (v.Length == 0) Config.Aliases.Remove(channel);
            else Config.Aliases[channel] = v;
            _tray.ApplyConfig();
            ShowPage(0);
        }

        private Panel BuildDeviceRow(ScanResult d, int w)
        {
            var row = new Panel { Width = w, Height = 44, BackColor = Theme.Content };
            bool isKeyboard = d.DeviceName != null && d.DeviceName.IndexOf("keyboard", StringComparison.OrdinalIgnoreCase) >= 0
                || (d.DeviceName != null && (d.DeviceName.IndexOf("Mechanical", StringComparison.OrdinalIgnoreCase) >= 0 || d.DeviceName.IndexOf("Keys", StringComparison.OrdinalIgnoreCase) >= 0));
            var icon = new IconView(isKeyboard ? (IconPaint)Icons.Keyboard : Icons.Mouse, 18) { Left = 2, Top = 13, Tint = Theme.Muted };
            var name = new Label { Text = d.DeviceName, AutoSize = true, Font = new Font("Segoe UI", 9.5f), ForeColor = Theme.Ink, Left = 30, Top = 13 };

            string chText = d.CurrentHost >= 0 ? Config.AliasFor(d.CurrentHost) : "?";
            var badge = new ThemedBadge { Text = "채널 " + (d.CurrentHost >= 0 ? (d.CurrentHost + 1).ToString() : "?"), Left = w - 220, Top = 10, Height = 24, AutoWidth = true };

            var switchBtn = new ThemedButton { Text = "전환", Width = 64, Height = 26, Left = w - 72, Top = 9 };
            switchBtn.Click += (s, e) => ShowDeviceSwitchMenu(d, switchBtn);

            row.Controls.Add(icon); row.Controls.Add(name); row.Controls.Add(badge); row.Controls.Add(switchBtn);
            var sep = new Panel { Left = 0, Top = 43, Width = w, Height = 1, BackColor = Theme.Divider };
            row.Controls.Add(sep);
            return row;
        }

        private void ShowDeviceSwitchMenu(ScanResult d, Control anchor)
        {
            var menu = new ContextMenuStrip();
            int hosts = d.HostCount > 0 ? d.HostCount : _tray.MaxHostCount();
            for (int h = 0; h < hosts; h++)
            {
                if (h == d.CurrentHost) continue;
                int target = h;
                var item = new ToolStripMenuItem(string.Format("채널 {0} · {1}", h + 1, Config.AliasFor(h)));
                item.Click += (s, e) => DoSwitchOne(d, target);
                menu.Items.Add(item);
            }
            if (menu.Items.Count == 0) menu.Items.Add(new ToolStripMenuItem("전환할 다른 채널 없음") { Enabled = false });
            menu.Show(anchor, new Point(0, anchor.Height));
        }

        private Panel BuildSwitchFooter(int w, MonitorLayoutPanel canvas)
        {
            var foot = new Panel { Width = w, Height = 86, BackColor = Theme.Content };
            var top = new Panel { Left = 0, Top = 0, Width = w, Height = 1, BackColor = Theme.Divider };
            foot.Controls.Add(top);

            var edgeIcon = new IconView(Icons.Edge, 16) { Left = 0, Top = 16, Tint = Theme.Muted };
            var edgeLbl = new Label { Text = "가장자리 자동 전환", AutoSize = true, Font = new Font("Segoe UI", 9.5f, FontStyle.Bold), ForeColor = Theme.Ink, Left = 24, Top = 15 };

            var dwellLbl = new Label { Text = "대기", AutoSize = true, ForeColor = Theme.Muted, Font = new Font("Segoe UI", 8.5f), Top = 18 };
            var dwellBtn = new ThemedButton { Text = DwellText(Config.DwellMs), Icon = null, Width = 78, Height = 26, Top = 14 };
            dwellBtn.Click += (s, e) => ShowDwellMenu(dwellBtn);
            var edgeToggle = new ThemedToggle { Checked = Config.EdgeSwitchEnabled, Top = 20 };
            edgeToggle.CheckedChanged += (s, e) => { Config.EdgeSwitchEnabled = edgeToggle.Checked; _tray.ApplyConfig(); };

            edgeToggle.Left = w - 28; dwellBtn.Left = edgeToggle.Left - 86; dwellLbl.Left = dwellBtn.Left - 30;
            foot.Controls.Add(edgeIcon); foot.Controls.Add(edgeLbl); foot.Controls.Add(dwellLbl); foot.Controls.Add(dwellBtn); foot.Controls.Add(edgeToggle);

            var guardChk = new ThemedToggle { Checked = Config.DragGuard, Top = 54 };
            var guardLbl = new Label { Text = "드래그 중 전환 안 함", AutoSize = true, ForeColor = Theme.Ink, Font = new Font("Segoe UI", 8.5f), Left = 34, Top = 56 };
            guardChk.Left = 0;
            guardChk.CheckedChanged += (s, e) => { Config.DragGuard = guardChk.Checked; _tray.ApplyConfig(); };

            var reset = new ThemedButton { Variant = ButtonVariant.Ghost, Text = "배치 초기화", Width = 90, Height = 26, Top = 54 };
            reset.Left = w - 90;
            reset.Click += (s, e) => { canvas.ClearPlacements(); };
            foot.Controls.Add(guardChk); foot.Controls.Add(guardLbl); foot.Controls.Add(reset);
            return foot;
        }

        private static string DwellText(int ms)
        {
            return (ms / 1000.0).ToString("0.##") + "초";
        }

        private void ShowDwellMenu(Control anchor)
        {
            var menu = new ContextMenuStrip();
            int[] opts = { 200, 330, 500, 800, 1000 };
            foreach (int ms in opts)
            {
                int v = ms;
                var item = new ToolStripMenuItem(DwellText(ms));
                item.Click += (s, e) => { Config.DwellMs = v; _tray.ApplyConfig(); ShowPage(0); };
                menu.Items.Add(item);
            }
            menu.Show(anchor, new Point(0, anchor.Height));
        }

        // ---- 설정 ----
        private Panel BuildSettingsPage()
        {
            var page = new Panel { Width = ContentW + 48, BackColor = Theme.Content };
            int W = ContentW;
            int y = 20;
            var title = Title("설정"); title.Location = new Point(24, y); page.Controls.Add(title);
            var sub = Sub("실행과 로그를 관리"); sub.Location = new Point(24, y + 28); page.Controls.Add(sub);
            y += 60;

            var g1 = GroupHead("테마"); g1.Location = new Point(24, y); page.Controls.Add(g1); y += 22;
            var themeCard = Card(24, y, W, 56);
            var seg = new SegmentedControl { Left = W - 230, Top = 13, Width = 216, Height = 30 };
            seg.SetItems(new[] { "시스템", "라이트", "다크" }, (int)Config.Theme);
            seg.SelectedChanged += (s, e) =>
            {
                Config.Theme = (ThemeMode)seg.SelectedIndex;
                Theme.SetMode(Config.Theme);
                _tray.ApplyConfig();
            };
            themeCard.Controls.Add(MakeRowLabel("테마", "시스템 설정을 따르거나 라이트·다크로 고정합니다.", 14));
            themeCard.Controls.Add(seg);
            page.Controls.Add(themeCard);
            y += 56 + 16;

            var g2 = GroupHead("실행"); g2.Location = new Point(24, y); page.Controls.Add(g2); y += 22;
            var runCard = Card(24, y, W, 2 * 60);
            AddSettingRow(runCard, 0, "Windows 시작 시 실행", "로그인하면 트레이에 자동으로 상주합니다.", false,
                Config.AutoStart, (on) => { Config.AutoStart = on; _tray.ApplyConfig(); });
            AddSettingRow(runCard, 60, "로그온 전에도 실행", "잠금 화면·로그인 전에도 전환이 동작합니다. 서비스 설치가 필요합니다.", true,
                Config.PreLogon, (on) =>
                {
                    Config.PreLogon = on;
                    _tray.ApplyConfig();
                    if (on) MessageBox.Show(this, "로그온 전 실행(서비스)은 준비 중입니다. 다음 판에서 제공됩니다.", "LogiFlow", MessageBoxButtons.OK, MessageBoxIcon.Information);
                });
            page.Controls.Add(runCard);
            y += 2 * 60 + 16;

            var g3 = GroupHead("전환 · 진단"); g3.Location = new Point(24, y); page.Controls.Add(g3); y += 22;
            var swCard = Card(24, y, W, 2 * 60);
            AddSettingRow(swCard, 0, "가장자리 자동 전환 사용", "모니터 변에 붙인 채널로 커서를 밀면 자동 전환합니다.", false,
                Config.EdgeSwitchEnabled, (on) => { Config.EdgeSwitchEnabled = on; _tray.ApplyConfig(); });
            AddSettingRow(swCard, 60, "진단 로그 남기기", "엣지 판정·전환 트리거를 로그 폴더에 기록합니다.", false,
                Config.DiagLogEnabled, (on) => { Config.DiagLogEnabled = on; Log.Enabled = on; _tray.ApplyConfig(); });
            page.Controls.Add(swCard);
            y += 2 * 60 + 20;

            page.Height = y;
            return page;
        }

        private Control MakeRowLabel(string lab, string desc, int top)
        {
            var p = new Panel { Left = 16, Top = top, Width = 420, Height = 40, BackColor = Theme.Section };
            var l1 = new Label { Text = lab, AutoSize = true, Font = new Font("Segoe UI", 9.5f, FontStyle.Bold), ForeColor = Theme.Ink, Left = 0, Top = 0 };
            var l2 = new Label { Text = desc, AutoSize = false, Font = new Font("Segoe UI", 8.5f), ForeColor = Theme.Muted, Left = 0, Top = 19, Width = 440, Height = 18 };
            p.Controls.Add(l1); p.Controls.Add(l2);
            return p;
        }

        private void AddSettingRow(Panel card, int top, string lab, string desc, bool advanced, bool on, Action<bool> onChange)
        {
            var row = new Panel { Left = 0, Top = top, Width = card.Width, Height = 60, BackColor = Theme.Section };
            var l1 = new Label { Text = lab, AutoSize = true, Font = new Font("Segoe UI", 9.5f, FontStyle.Bold), ForeColor = Theme.Ink, Left = 16, Top = 12 };
            row.Controls.Add(l1);
            if (advanced)
            {
                var adv = new ThemedBadge { Text = "고급", Left = l1.Right + 8, Top = 11, Height = 18, AutoWidth = true, UseAccent = true };
                row.Controls.Add(adv);
            }
            var l2 = new Label { Text = desc, AutoSize = false, Font = new Font("Segoe UI", 8.5f), ForeColor = Theme.Muted, Left = 16, Top = 32, Width = card.Width - 90, Height = 18 };
            row.Controls.Add(l2);
            var tg = new ThemedToggle { Checked = on, Left = card.Width - 44, Top = 22 };
            tg.CheckedChanged += (s, e) => onChange(tg.Checked);
            row.Controls.Add(tg);
            if (top > 0)
            {
                var sep = new Panel { Left = 16, Top = 0, Width = card.Width - 32, Height = 1, BackColor = Theme.Divider };
                row.Controls.Add(sep);
            }
            card.Controls.Add(row);
        }

        // ---- 진단 ----
        private Panel BuildDiagPage()
        {
            var page = new Panel { Width = ContentW + 48, BackColor = Theme.Content };
            int W = ContentW;
            int y = 20;
            var title = Title("진단"); title.Location = new Point(24, y); page.Controls.Add(title);
            var sub = Sub("기기별 채널·전환 인덱스 확인"); sub.Location = new Point(24, y + 28); page.Controls.Add(sub);

            var showAll = new ThemedToggle { Checked = _showAllDiag, Top = y + 4 };
            var showAllLbl = new Label { Text = "모두 보기", AutoSize = true, ForeColor = Theme.Ink, Font = new Font("Segoe UI", 8.5f), Top = y + 6 };
            var refreshBtn = new ThemedButton { Text = "새로 고침", Icon = Icons.Refresh, Width = 104, Height = 28, Top = y };
            var logBtn = new ThemedButton { Text = "로그 폴더 열기", Icon = Icons.Folder, Width = 128, Height = 28, Top = y };
            logBtn.Left = 24 + W - 128; refreshBtn.Left = logBtn.Left - 112; showAll.Left = refreshBtn.Left - 40; showAllLbl.Left = showAll.Left - 56;
            showAll.CheckedChanged += (s, e) => { _showAllDiag = showAll.Checked; ShowPage(2); };
            refreshBtn.Click += (s, e) => _tray.Rescan();
            logBtn.Click += (s, e) => OpenLogFolder();
            page.Controls.Add(showAllLbl); page.Controls.Add(showAll); page.Controls.Add(refreshBtn); page.Controls.Add(logBtn);
            y += 58;

            var all = _tray.LastResults;
            var shown = new List<ScanResult>();
            foreach (var r in all) if (_showAllDiag || r.ChangeHostSupported) shown.Add(r);
            int switchableCount = _tray.Switchable().Count;

            var secLbl = SectionLabel("스캔된 기기", "전환 가능 " + switchableCount + "개"); secLbl.Location = new Point(24, y); page.Controls.Add(secLbl);
            y += 24;

            // header
            var head = BuildDiagHeader(W); head.Location = new Point(24, y); page.Controls.Add(head); y += 28;
            if (shown.Count == 0)
            {
                var none = new Label { Text = "표시할 기기가 없습니다. [새로 고침] 하세요.", AutoSize = true, ForeColor = Theme.Muted, Location = new Point(24, y) };
                page.Controls.Add(none); y += 30;
            }
            foreach (var r in shown)
            {
                var row = BuildDiagRow(r, W); row.Location = new Point(24, y); page.Controls.Add(row); y += 40;
            }
            y += 6;

            var noteIcon = new IconView(Icons.Plug, 14) { Left = 24, Top = y + 1, Tint = Theme.Muted };
            var note = new Label { Text = "기기 정보는 저장되어 매번 재검색하지 않음 · \"모두 보기\"를 켜면 빈 슬롯·리시버도 함께 표시", AutoSize = true, ForeColor = Theme.Muted, Font = new Font("Segoe UI", 8.5f), Left = 44, Top = y };
            page.Controls.Add(noteIcon); page.Controls.Add(note);
            y += 40;

            page.Height = y;
            return page;
        }

        private bool _showAllDiag;

        private Panel BuildDiagHeader(int w)
        {
            var p = new Panel { Width = w, Height = 26, BackColor = Theme.Content };
            AddCol(p, "기기", 0, (int)(w * 0.30));
            AddCol(p, "VID:PID", (int)(w * 0.30), (int)(w * 0.18));
            AddCol(p, "연결", (int)(w * 0.48), (int)(w * 0.22));
            AddCol(p, "0x1814 인덱스", (int)(w * 0.70), (int)(w * 0.18));
            AddCol(p, "채널", (int)(w * 0.88), (int)(w * 0.12));
            var sep = new Panel { Left = 0, Top = 25, Width = w, Height = 1, BackColor = Theme.Divider };
            p.Controls.Add(sep);
            return p;
        }

        private void AddCol(Panel p, string text, int x, int wcol)
        {
            p.Controls.Add(new Label { Text = text, AutoSize = false, Width = wcol, Height = 18, Left = x, Top = 4, Font = new Font("Segoe UI", 8f, FontStyle.Bold), ForeColor = Theme.Muted });
        }

        private Panel BuildDiagRow(ScanResult r, int w)
        {
            var p = new Panel { Width = w, Height = 40, BackColor = Theme.Content };
            bool isKeyboard = r.DeviceName != null && (r.DeviceName.IndexOf("Mechanical", StringComparison.OrdinalIgnoreCase) >= 0 || r.DeviceName.IndexOf("Keys", StringComparison.OrdinalIgnoreCase) >= 0 || r.DeviceName.IndexOf("keyboard", StringComparison.OrdinalIgnoreCase) >= 0);
            var icon = new IconView(isKeyboard ? (IconPaint)Icons.Keyboard : Icons.Mouse, 17) { Left = 0, Top = 11, Tint = Theme.Muted };
            var name = new Label { Text = r.DeviceName, AutoSize = false, Width = (int)(w * 0.30) - 26, Height = 18, Left = 26, Top = 11, Font = new Font("Segoe UI", 9f), ForeColor = Theme.Ink };
            p.Controls.Add(icon); p.Controls.Add(name);
            p.Controls.Add(ColLabel(r.VidPid, (int)(w * 0.30), (int)(w * 0.18), Theme.Muted));
            p.Controls.Add(ColLabel(r.ConnectionType, (int)(w * 0.48), (int)(w * 0.22), Theme.Ink));
            p.Controls.Add(ColLabel(r.ChangeHostSupported ? string.Format("0x{0:X2}", r.ChangeHostIndex) : "미지원", (int)(w * 0.70), (int)(w * 0.18), Theme.Ink));
            var badge = new ThemedBadge { Text = r.CurrentHost >= 0 ? "채널 " + (r.CurrentHost + 1) : "-", Left = (int)(w * 0.88), Top = 8, Height = 22, AutoWidth = true };
            p.Controls.Add(badge);
            var sep = new Panel { Left = 0, Top = 39, Width = w, Height = 1, BackColor = Theme.Divider };
            p.Controls.Add(sep);
            return p;
        }

        private Label ColLabel(string text, int x, int wcol, Color c)
        {
            return new Label { Text = text, AutoSize = false, Width = wcol, Height = 18, Left = x, Top = 11, Font = new Font("Segoe UI", 9f), ForeColor = c };
        }

        // ---- 전환 동작 ----
        private void DoSwitchOne(ScanResult device, int targetHost)
        {
            if (_busy) return;
            if (MessageBox.Show(this, string.Format("{0} 를 채널 {1} ({2}) 로 전환합니다.\r\n이 PC 에서 기기가 사라집니다. 진행할까요?",
                device.DeviceName, targetHost + 1, Config.AliasFor(targetHost)), "LogiFlow 전환",
                MessageBoxButtons.YesNo, MessageBoxIcon.Warning) != DialogResult.Yes) return;
            _busy = true;
            var t = new Thread(() =>
            {
                bool ok = HostSwitcher.SwitchOne(device, targetHost, Log);
                try { BeginInvoke((Action)(() => { _busy = false; MessageBox.Show(this, ok ? "전환 명령을 보냈습니다." : "전환 실패 — 로그를 확인하세요.", "LogiFlow", MessageBoxButtons.OK, MessageBoxIcon.Information); })); }
                catch { _busy = false; }
            });
            t.IsBackground = true; t.Start();
        }

        private void OpenLogFolder()
        {
            try
            {
                Directory.CreateDirectory(Log.LogDirectory);
                Process.Start("explorer.exe", "\"" + Log.LogDirectory + "\"");
            }
            catch (Exception ex)
            {
                MessageBox.Show(this, "로그 폴더를 열지 못했습니다:\r\n" + ex.Message, "LogiFlow", MessageBoxButtons.OK, MessageBoxIcon.Warning);
            }
        }

        public void ShowSettings()
        {
            if (!Visible) Show();
            if (WindowState == FormWindowState.Minimized) WindowState = FormWindowState.Normal;
            Activate(); BringToFront();
            ShowPage(_pageIndex);
        }

        public void OnStateUpdated()
        {
            if (IsDisposed) return;
            ShowPage(_pageIndex);
        }

        private void OnFormClosing(object sender, FormClosingEventArgs e)
        {
            if (e.CloseReason == CloseReason.UserClosing)
            {
                e.Cancel = true;
                Hide();
            }
        }

        protected override void WndProc(ref Message m)
        {
            const int WM_DEVICECHANGE = 0x0219;
            if (m.Msg == WM_DEVICECHANGE)
            {
                try { _tray.OnDeviceChanged(); } catch { }
            }
            base.WndProc(ref m);
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing) Theme.Changed -= OnThemeChanged;
            base.Dispose(disposing);
        }
    }
}
