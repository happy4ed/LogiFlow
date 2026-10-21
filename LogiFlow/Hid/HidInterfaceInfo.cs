namespace LogiFlow.Hid
{
    /// <summary>열거된 HID 인터페이스 1개의 정적 속성.</summary>
    internal sealed class HidInterfaceInfo
    {
        public string Path;
        public ushort VendorId;
        public ushort ProductId;
        public ushort UsagePage;
        public ushort Usage;
        public int InputReportByteLength;
        public int OutputReportByteLength;
        public string ProductString;

        public string VidPid
        {
            get { return string.Format("{0:X4}:{1:X4}", VendorId, ProductId); }
        }
    }
}
