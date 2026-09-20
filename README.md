# Flipsy

초경량 Pure Rust 네이티브 오라클 클라이언트 및 실행계획(XPlan) 시각화 도구.

Webview(Chromium)나 Node.js 런타임을 완전히 배제하고, 순수 Rust 그래픽 파이프라인(`eframe` / `egui`)으로 빌드되어 **3.7MB 단일 바이너리**와 **100MB 미만의 메모리 점유율**로 동작합니다.

---

## 1. 주요 기능 및 특징

- **Zero Webview & Pure Rust Native**:
  - 일체의 Electron/Tauri Webview 오버헤드 없이 OS 그래픽 파이프라인에 직접 렌더링.
  - 릴리스 바이너리 크기 **3.7 MB**, Working Set 메모리 약 **100 MB** 내외.
- **카카오톡 스타일 독립 다중 창 (Multi-Viewport)**:
  - 서버 목록 창과 개별 데이터베이스 로그인 창이 독립된 자식 뷰포트로 분리되어 기동.
- **tnsnames.ora 실시간 연동 및 인앱 편집기**:
  - 로컬 `tnsnames.ora` 파일 파싱, 접속 대상 동적 로드 및 GUI 기반 실시간 편집/저장 지원.
- **Oracle 26ai Free 실시간 세션 연동**:
  - Docker 컨테이너(`oracle23ai`)와의 프로세스 통신을 통해 실제 세션 인증 및 쿼리 파이프라인 구동.
  - 잘못된 자격 증명 시 오라클 엔진의 실제 `ORA-01017` 에러 리턴 및 폼 검증.
  - `v$version`, 세션 메타데이터(`USERENV`), 실시간 DDL/DML 결과 출력.
- **실행계획(XPlan) 시각화 및 병목 탐지**:
  - `EXPLAIN PLAN` 및 `DBMS_XPLAN.DISPLAY()` 파싱.
  - 대량 루프 조인(Starts 왜곡) 및 풀스캔 병목 노드 식별.

---

## 2. 프로젝트 아키텍처

```text
src/
├── main.rs         # 앱 진입점 및 윈도우 네이티브 F 아이콘 래스터라이저
├── app.rs          # 앱 상태 머신 (ServerList <-> Workspace)
├── models.rs       # ConnectionConfig, PlanNode, QueryResult 데이터 모델
├── db/
│   ├── session.rs  # Oracle 26ai 실시간 쿼리 실행기 & CSV 파서
│   ├── tns.rs      # tnsnames.ora 구문 파서 및 직렬화
│   └── mock.rs     # 150K Starts 튜닝 분석용 시뮬레이션 엔진
└── ui/
    ├── server_list.rs # 서버 목록 메인 윈도우 및 로그인 자식 뷰포트
    ├── tns_editor.rs  # 인앱 tnsnames.ora 편집기 모달
    ├── editor.rs      # SQL 구문 에디터 (Ctrl+Enter, F10 단축키)
    ├── grid.rs        # 쿼리 결과 그리드 뷰 (Roundtrip 지표 출력)
    └── plan_tree.rs   # XPlan 계층 구조 시각화 트리
```

---

## 3. 빌드 및 실행

### 필수 요구사항
- Rust 1.75+ (Cargo)
- (선택) Oracle Database 26ai Free Docker 컨테이너

### 빌드 및 실행
```bash
# 개발 모드 실행
cargo run

# 최적화 릴리스 빌드 (LTO, Strip 적용)
cargo build --release
```

최적화 빌드 결과물은 `target/release/flipsy.exe`에 생성됩니다.
