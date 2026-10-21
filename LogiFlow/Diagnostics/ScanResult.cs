namespace LogiFlow.Diagnostics
{
    /// <summary>발견된 HID++ 장치 1개(= 인터페이스 × deviceIndex)의 진단 결과.</summary>
    internal sealed class ScanResult
    {
        public string DeviceName;          // HID++ 0x0005 또는 Windows 제품 문자열
        public string VidPid;              // 046D:XXXX
        public string Path;                // HID 인터페이스 경로
        public string ConnectionType;      // 직접(USB/BT) / 리시버 뒤(index N)
        public byte DeviceIndex;           // 0xFF 또는 1~6
        public bool ChangeHostSupported;   // 0x1814 지원 여부
        public byte ChangeHostIndex;       // 런타임 조회된 0x1814 feature 인덱스
        public int HostCount = -1;         // 호스트(채널) 개수
        public int CurrentHost = -1;       // 현재 호스트(0-base)
        public string Note;                // 비고(오류·미지원 등)
    }
}
