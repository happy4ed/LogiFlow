using System;

namespace LogiFlow.Handoff
{
    // 출처: LocalDex Shared/InputHandoff.cs (DexManager.Models) 를 LogiFlow 로 이식.
    // 라이선스 양쪽 모두 회장님 소유라 복제 가능. 폰 핸드오프 도메인 제거, 채널 전환에 맞게 적응.

    /// <summary>PC 데스크톱에서 다른 채널 PC 가 놓인 변.</summary>
    public enum HandoffEdge
    {
        Left = 0,
        Right = 1,
        Top = 2,
        Bottom = 3
    }

    /// <summary>논리 좌표 사각형(모니터·박스).</summary>
    public struct HandoffRect
    {
        public HandoffRect(int left, int top, int width, int height)
        {
            Left = left;
            Top = top;
            Width = Math.Max(1, width);
            Height = Math.Max(1, height);
        }

        public int Left { get; private set; }
        public int Top { get; private set; }
        public int Width { get; private set; }
        public int Height { get; private set; }
        public int Right { get { return Left + Width; } }
        public int Bottom { get { return Top + Height; } }
    }
}
