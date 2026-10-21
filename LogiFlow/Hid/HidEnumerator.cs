using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
using Microsoft.Win32.SafeHandles;
using LogiFlow.Native;

namespace LogiFlow.Hid
{
    /// <summary>
    /// SetupAPI 로 시스템의 HID 인터페이스를 열거한다. 읽기 전용.
    /// </summary>
    internal static class HidEnumerator
    {
        /// <summary>
        /// 모든 HID 인터페이스를 열거하되, 필터로 VID 와 usagePage 를 지정할 수 있다.
        /// vendorId 0 이면 전체 VID, usagePage 0 이면 전체 usagePage.
        /// </summary>
        public static List<HidInterfaceInfo> Enumerate(ushort vendorIdFilter, ushort usagePageFilter)
        {
            var result = new List<HidInterfaceInfo>();

            Guid hidGuid;
            NativeMethods.HidD_GetHidGuid(out hidGuid);

            IntPtr deviceInfoSet = NativeMethods.SetupDiGetClassDevs(
                ref hidGuid, IntPtr.Zero, IntPtr.Zero,
                NativeMethods.DIGCF_PRESENT | NativeMethods.DIGCF_DEVICEINTERFACE);

            if (deviceInfoSet == IntPtr.Zero || deviceInfoSet == new IntPtr(-1))
            {
                return result;
            }

            try
            {
                var interfaceData = new NativeMethods.SP_DEVICE_INTERFACE_DATA();
                interfaceData.cbSize = Marshal.SizeOf(typeof(NativeMethods.SP_DEVICE_INTERFACE_DATA));

                int index = 0;
                while (NativeMethods.SetupDiEnumDeviceInterfaces(
                    deviceInfoSet, IntPtr.Zero, ref hidGuid, index, ref interfaceData))
                {
                    index++;

                    string path = GetDevicePath(deviceInfoSet, ref interfaceData);
                    if (string.IsNullOrEmpty(path))
                    {
                        continue;
                    }

                    // 경로 문자열에서 먼저 VID/PID 를 뽑아 값싼 1차 필터(열기 전).
                    ParseVidPidFromPath(path, out ushort pathVid, out ushort pathPid);
                    if (vendorIdFilter != 0 && pathVid != 0 && pathVid != vendorIdFilter)
                    {
                        continue;
                    }

                    HidInterfaceInfo info = TryReadInterface(path);
                    if (info == null)
                    {
                        // caps/attrs 를 못 읽어도(다른 프로세스가 선점 등) 버리지 않고
                        // 경로에서 뽑은 최소 정보로 기록 — 진단에서 보이게 한다.
                        info = new HidInterfaceInfo
                        {
                            Path = path,
                            VendorId = pathVid,
                            ProductId = pathPid,
                            UsagePage = 0,
                            Usage = 0,
                            CapsRead = false,
                        };
                    }

                    if (vendorIdFilter != 0 && info.VendorId != vendorIdFilter && pathVid != vendorIdFilter)
                    {
                        continue;
                    }
                    if (usagePageFilter != 0 && info.UsagePage != usagePageFilter)
                    {
                        continue;
                    }

                    result.Add(info);
                }
            }
            finally
            {
                NativeMethods.SetupDiDestroyDeviceInfoList(deviceInfoSet);
            }

            return result;
        }

        private static string GetDevicePath(IntPtr deviceInfoSet, ref NativeMethods.SP_DEVICE_INTERFACE_DATA interfaceData)
        {
            int requiredSize = 0;
            NativeMethods.SetupDiGetDeviceInterfaceDetail(
                deviceInfoSet, ref interfaceData, IntPtr.Zero, 0, ref requiredSize, IntPtr.Zero);

            if (requiredSize == 0)
            {
                return null;
            }

            var detail = new NativeMethods.SP_DEVICE_INTERFACE_DETAIL_DATA();
            // cbSize 는 구조체 헤더 크기: 64비트는 8, 32비트는 6.
            detail.cbSize = IntPtr.Size == 8 ? 8 : 6;

            if (!NativeMethods.SetupDiGetDeviceInterfaceDetail(
                deviceInfoSet, ref interfaceData, ref detail, requiredSize, ref requiredSize, IntPtr.Zero))
            {
                return null;
            }

            return detail.DevicePath;
        }

        private static HidInterfaceInfo TryReadInterface(string path)
        {
            // 열거 단계에서는 공유 모드로 열어 다른 프로세스(로지텍 Options 등)와 충돌하지 않게 한다.
            SafeFileHandle handle = NativeMethods.CreateFile(
                path, 0,
                NativeMethods.FILE_SHARE_READ | NativeMethods.FILE_SHARE_WRITE,
                IntPtr.Zero, NativeMethods.OPEN_EXISTING, 0, IntPtr.Zero);

            if (handle.IsInvalid)
            {
                handle.Dispose();
                return null;
            }

            try
            {
                var attrs = new NativeMethods.HIDD_ATTRIBUTES();
                attrs.Size = Marshal.SizeOf(typeof(NativeMethods.HIDD_ATTRIBUTES));
                if (!NativeMethods.HidD_GetAttributes(handle, ref attrs))
                {
                    return null;
                }

                IntPtr preparsed;
                if (!NativeMethods.HidD_GetPreparsedData(handle, out preparsed))
                {
                    return null;
                }

                NativeMethods.HIDP_CAPS caps;
                try
                {
                    if (NativeMethods.HidP_GetCaps(preparsed, out caps) != /*HIDP_STATUS_SUCCESS*/ 0x00110000)
                    {
                        return null;
                    }
                }
                finally
                {
                    NativeMethods.HidD_FreePreparsedData(preparsed);
                }

                var info = new HidInterfaceInfo
                {
                    Path = path,
                    VendorId = attrs.VendorID,
                    ProductId = attrs.ProductID,
                    UsagePage = caps.UsagePage,
                    Usage = caps.Usage,
                    InputReportByteLength = caps.InputReportByteLength,
                    OutputReportByteLength = caps.OutputReportByteLength,
                    ProductString = ReadProductString(handle),
                    CapsRead = true,
                };
                return info;
            }
            finally
            {
                handle.Dispose();
            }
        }

        // 경로 예: \\?\hid#vid_046d&pid_c548&mi_01&col01#...  에서 vid/pid 추출.
        private static void ParseVidPidFromPath(string path, out ushort vid, out ushort pid)
        {
            vid = 0;
            pid = 0;
            if (string.IsNullOrEmpty(path)) return;
            string p = path.ToLowerInvariant();
            vid = ExtractHex(p, "vid_");
            pid = ExtractHex(p, "pid_");
        }

        private static ushort ExtractHex(string lowerPath, string key)
        {
            int i = lowerPath.IndexOf(key, StringComparison.Ordinal);
            if (i < 0) return 0;
            i += key.Length;
            if (i + 4 > lowerPath.Length) return 0;
            string hex = lowerPath.Substring(i, 4);
            ushort val;
            return ushort.TryParse(hex, System.Globalization.NumberStyles.HexNumber,
                System.Globalization.CultureInfo.InvariantCulture, out val) ? val : (ushort)0;
        }

        private static string ReadProductString(SafeFileHandle handle)
        {
            try
            {
                var buffer = new byte[256];
                if (NativeMethods.HidD_GetProductString(handle, buffer, buffer.Length))
                {
                    string s = Encoding.Unicode.GetString(buffer);
                    int nul = s.IndexOf('\0');
                    if (nul >= 0)
                    {
                        s = s.Substring(0, nul);
                    }
                    return s.Trim();
                }
            }
            catch
            {
                // 제품 문자열은 선택 정보 — 실패해도 무시.
            }
            return null;
        }
    }
}
