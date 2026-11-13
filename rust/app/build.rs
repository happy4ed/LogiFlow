//! 실행 파일 리소스 — 앱 아이콘(리소스 1 = logiflow.ico, 트레이·창 공용) · 버전 정보 · 매니페스트.
//! 매니페스트: asInvoker · Common Controls v6(Windows 기본 대화상자·메뉴 모양) · DPI 설정 없음
//! (상주 프로세스는 지금 판처럼 DPI 미인식 → 좌표 1:1, 설정 창 프로세스만 실행 중에 배율 인식으로 바뀜).

use std::path::PathBuf;

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        return;
    }
    use embed_manifest::manifest::{ActiveCodePage, DpiAwareness, SupportedOS};
    let m = embed_manifest::new_manifest("LogiFlowRs.app")
        .dpi_awareness(DpiAwareness::UnawareByDefault)
        .active_code_page(ActiveCodePage::Legacy)
        .supported_os(SupportedOS::Windows7..=SupportedOS::Windows10);
    embed_manifest::embed_manifest(m).expect("manifest");

    // .rc 는 리눅스에서 형식 검사(cargo check)할 땐 건너뛴다 — 리소스 컴파일러는 Windows CI 에만 있다.
    let host = std::env::var("HOST").unwrap_or_default();
    if !host.contains("windows") {
        return;
    }
    let ver = std::env::var("CARGO_PKG_VERSION").unwrap();
    // 0.16.0-rs.N → 0,16,0,N
    let (core, pre) = ver.split_once('-').unwrap_or((&ver, ""));
    let mut nums: Vec<u32> = core.split('.').map(|s| s.parse().unwrap_or(0)).collect();
    nums.resize(3, 0);
    let n = pre.rsplit('.').next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let fv = format!("{},{},{},{}", nums[0], nums[1], nums[2], n);
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let ico = manifest_dir.join("../../LogiFlow/Resources/logiflow.ico").canonicalize().expect("logiflow.ico");
    let ico_s = ico.display().to_string().replace('\\', "\\\\");
    let rc = format!(
        r#"#pragma code_page(65001)
1 ICON "{ico}"
1 VERSIONINFO
FILEVERSION {fv}
PRODUCTVERSION {fv}
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "041204B0"
    BEGIN
      VALUE "CompanyName", "happy4ed"
      VALUE "FileDescription", "LogiFlow (러스트 시험판)"
      VALUE "FileVersion", "{ver}"
      VALUE "InternalName", "LogiFlowRs"
      VALUE "LegalCopyright", "Copyright (c) 2026 happy4ed · MIT"
      VALUE "OriginalFilename", "LogiFlowRs.exe"
      VALUE "ProductName", "LogiFlow"
      VALUE "ProductVersion", "{ver}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x412, 1200
  END
END
"#,
        ico = ico_s
    );
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("LogiFlowRs.rc");
    std::fs::write(&out, rc).unwrap();
    embed_resource::compile(&out, embed_resource::NONE).manifest_optional().expect("resources");
    println!("cargo:rerun-if-changed=../../LogiFlow/Resources/logiflow.ico");
}
