using System;
using System.Drawing;
using System.IO;
using System.Reflection;

namespace LogiFlow.Ui
{
    /// <summary>임베디드 앱 아이콘(시안 "두 화면 흐름" 마크) 로더. 창·트레이 공용.</summary>
    public static class AppIcon
    {
        private static Icon _cached;

        public static Icon Get()
        {
            if (_cached != null) return _cached;
            try
            {
                Assembly asm = Assembly.GetExecutingAssembly();
                using (Stream s = asm.GetManifestResourceStream("LogiFlow.Resources.logiflow.ico"))
                {
                    if (s != null)
                    {
                        _cached = new Icon(s);
                        return _cached;
                    }
                }
            }
            catch
            {
            }
            _cached = SystemIcons.Application;
            return _cached;
        }
    }
}
