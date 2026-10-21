using System;

namespace LogiFlow.Hidpp
{
    /// <summary>HID++ 장치가 반환한 오류 응답.</summary>
    internal sealed class HidppErrorException : Exception
    {
        public int ErrorCode { get; private set; }

        public HidppErrorException(int errorCode, string message)
            : base(message)
        {
            ErrorCode = errorCode;
        }

        public static string Describe(int code)
        {
            switch (code)
            {
                case 0: return "NoError";
                case 1: return "Unknown";
                case 2: return "InvalidArgument";
                case 3: return "OutOfRange";
                case 4: return "HardwareError";
                case 5: return "LogitechInternal";
                case 6: return "InvalidFeatureIndex";
                case 7: return "InvalidFunctionId";
                case 8: return "Busy";
                case 9: return "Unsupported";
                default: return "Error(0x" + code.ToString("X2") + ")";
            }
        }
    }
}
