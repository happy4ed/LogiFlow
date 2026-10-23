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
        public bool CapsRead;   // caps/attrs 를 실제로 읽었는지(false면 경로에서만 추출)
        public byte[] DeviceIndexHint;  // 핑할 deviceIndex 만 알 때(리눅스 hidraw). null 이면 0xFF+1~6 전부

        public string VidPid
        {
            get { return string.Format("{0:X4}:{1:X4}", VendorId, ProductId); }
        }
    }
}
