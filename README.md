# Flipsy (플립시)

**초경량 Pure Rust 네이티브 오라클 클라이언트 & 실행계획(XPlan) 시각화 도구**

SQL Developer나 Toad 같이 무겁고 기동이 느린 오라클 접속 툴 대신, **0초 즉시 기동**, **5MB 단일 바이너리**, **100MB 미만의 메모리 점유율**로 실무 운영 편의성을 극대화하기 위해 설계된 순수 Rust 네이티브 프로그램입니다.

---

## 1. 개발 목적 및 철학

* **운영 편의성과 민첩성 (Agility)**: 수백 MB에서 수 GB에 달하는 Java JVM(SQL Developer)이나 무거운 상용 툴을 띄우지 않고, 클릭 즉시 켜지는 포터블 도구 제공.
* **Zero Webview & Pure Rust Native**: Electron, Chromium, Node.js 런타임을 완전히 배제하고 OS 네이티브 그래픽 파이프라인(`eframe` / `egui`)에 직접 렌더링.
* **단일 파일 배포 (Zero Dependency)**: 외부 런타임 설치 없이 단 하나의 실행 파일(`flipsy.exe`)로 어디서든 즉시 동작.

---

## 2. 주요 기능 및 특징

### 🚀 초경량 네이티브 엔진 & 운영 최적화
* **단일 세션 쿼리 + XPlan 파이프라인 (Single-shot Execution)**:
  - 쿼리 실행과 `DBMS_XPLAN.DISPLAY_CURSOR(ALLSTATS LAST)` 수집을 단일 SQL*Plus 세션으로 통합하여 프로세스 기동 및 로그인 오버헤드를 절반으로 단축.
* **유휴(Idle) CPU 점유율 0% 유지**:
  - 즉시 모드(Immediate Mode) GUI 환경에서 정규식 정적 캐싱(`OnceLock`), SQL 텍스트 변경 감지 캐싱(`ensure_cache`), 아이콘 래스터라이징 캐싱을 적용하여 프레임당 힙 할당 제로화.
* **tnsnames.ora 자동 탐색 (포터블 배포 지원)**:
  - 환경변수 `%TNS_ADMIN%`, 실행 파일(`.exe`) 동일 디렉토리, 현재 작업 경로, `%ORACLE_HOME%`을 우선순위별로 자동 탐색하여 어디서든 서버 목록 즉시 인식.
  - 인앱 실시간 tnsnames.ora 편집기 내장.
* **결과 그리드 원클릭 엑셀(TSV) 복사**:
  - 쿼리 결과 데이터를 탭 구분자(TSV) 형식으로 클립보드에 복사하여 Excel에 `Ctrl+V`로 즉시 셀 구분 표 형태로 붙여넣기 지원.
  - 100~500건 단위 페이징 슬라이싱을 통한 대용량 데이터 메모리 보호.

### 🎨 카카오톡 스타일 독립 다중 창 (Multi-Viewport)
* 서버 목록 창과 개별 데이터베이스 로그인 창이 독립된 자식 뷰포트로 분리 기동.
* 메인 창을 유지하면서 여러 대상 서버를 편리하게 선택/전환 가능.

### ⚡ 실행계획(XPlan) 시각화 및 병목 탐지
* **1번 방식 (F10)**: `EXPLAIN PLAN FOR` 기반 옵티마이저 예측 계획 생성.
* **2번 방식 (Ctrl+Enter)**: `ALTER SESSION SET STATISTICS_LEVEL = ALL` 실측치 기반 `DBMS_XPLAN.DISPLAY_CURSOR` 수집.
* Starts 왜곡(대량 루프 조인), 과도한 버퍼 I/O, 풀 테이블 스캔 등 병목 노드 자동 식별.

---

## 3. 하드웨어 연산 분담 구조 (CPU vs GPU)

Flipsy는 컴퓨터 자원을 가장 효율적인 형태로 분담하여 동작합니다:

```text
[ Flipsy 하드웨어 분담 구조 ]

1. CPU (순수 Rust 고속 연산 바운드)
   ├── SQL 토크나이징 및 포맷팅 (Line-up / F8)
   ├── 정규식 기반 테이블/조인/바인드 구조 분석 (Figure, Bind)
   ├── tnsnames.ora 블록 스캐닝
   ├── 오라클 CSV 응답 스트리밍 파싱
   └── XPlan 실행계획 계층 트리(PlanNode) 구성

2. GPU (OS 그래픽 하드웨어 가속 바운드)
   └── egui / eframe 그래픽 파이프라인
       ├── 텍스트 폰트 글리프 래스터라이징
       ├── 윈도우 UI 패널, 카드 박스, 테두리 벡터 렌더링 (DirectX / Vulkan)
       └── 60 FPS 화면 버퍼 스왑
```

* **CPU**: 텍스트 분석 및 트리 순회는 순차적 의존성과 분기(Branching)가 많은 작업이므로, 초고속 캐시를 갖춘 CPU가 마이크로초 단위로 처리합니다.
* **GPU**: 화면을 그리는 벡터/픽셀 렌더링에만 순수 하드웨어 가속을 사용하여 렌더링으로 인한 CPU 부하를 없앱니다.

---

## 4. 파싱 및 구조 분석 철학 (AST vs 경량 토크나이저)

Flipsy가 거대한 컴파일러 AST(추상 구문 트리) 파서를 쓰지 않고 **경량 토크나이저 + 상태 머신 + 정규식**을 채택한 기술적 배경:

1. **바이너리 비대화 방지**:
   - `sqlparser-rs`나 `ANTLR` 같은 무거운 파서 라이브러리 의존성을 배제하여 5MB 단일 바이너리 크기 유지.
2. **오라클 비표준/특수 문법 호환성**:
   - 엄격한 AST 파서는 복잡한 힌트(`/*+ GATHER_PLAN_STATISTICS */`), `(+)` 구식 외부조인, 계층 쿼리(`CONNECT BY`) 등에서 구문 오류(Syntax Error)를 내며 멈추기 쉽습니다.
   - Flipsy의 1차원 토크나이저와 패턴 스캐너는 문법의 완전성과 무관하게 **식별자를 100% 보존하면서 필요한 구조(테이블, 조인, 바인드)만 유연하게 추출**합니다.

---

## 5. 로드맵: 복잡한 WITH절/서브쿼리 다이어그램 (Figure 2.0)

복잡한 다중 CTE(`WITH`), 인라인 뷰, 서브쿼리가 얽힌 대형 쿼리를 한눈에 파악할 수 있는 **데이터 흐름 다이어그램(Data Flow DAG)** 고도화 계획:

* **스코프 인식 괄호 스택 파서(Scoped Block Parser)**:
  - 괄호 밸런싱을 통해 각 CTE와 서브쿼리 블록의 계층적 스코프를 분리.
* **유향 비순환 그래프(DAG) 모델링**:
  - `물리 테이블` $\rightarrow$ `1차 CTE` $\rightarrow$ `파생 CTE/서브쿼리` $\rightarrow$ `최종 메인 SELECT`로 이어지는 데이터 흐름 및 조인 관계 매핑.
* **egui 네이티브 벡터 다이어그램 렌더링**:
  - `egui::Painter`를 통한 노드 박스(물리 테이블, CTE, 서브쿼리) 및 베지어 곡선 연결선/화살표 인터랙티브 시각화.

---

## 6. 프로젝트 디렉토리 구조

```text
src/
├── main.rs         # 앱 진입점 및 캐시된 F 아이콘 래스터라이저
├── app.rs          # 앱 상태 머신 (ServerList <-> Workspace)
├── models.rs       # ConnectionConfig, PlanNode, QueryResult, QueryStructure 데이터 모델
├── db/
│   ├── session.rs  # 단일 파이프라인 쿼리/XPlan 실행기 & 프로세스 브리지
│   ├── tns.rs      # tnsnames.ora 자동 탐색기 & 파서
│   ├── sql_utils.rs# SQL 토크나이저 포맷터, 바인드 추출기, 정규식 캐싱 구조 분석기
│   ├── csv_parser.rs# SQL*Plus CSV 출력 스트리밍 파서
│   ├── xplan.rs    # DBMS_XPLAN 계층 구조 파서 & 병목 탐지기
│   └── mock.rs     # 150K Starts 튜닝 분석용 오프라인 시뮬레이션 엔진
└── ui/
    ├── server_list.rs # 서버 목록 메인 창 및 로그인 독립 뷰포트
    ├── tns_editor.rs  # 인앱 tnsnames.ora 편집기 모달
    ├── editor.rs      # 캐싱 기반 SQL 에디터 (Ctrl+Enter, F10, F8)
    ├── grid.rs        # 쿼리 결과 그리드 (엑셀 TSV 복사, 페이징)
    └── plan_tree.rs   # XPlan 계층 구조 단일 패널 트리 뷰
```

---

## 7. 환경 설정 (선택적 환경 변수)

| 환경 변수 | 기본값 | 설명 |
| :--- | :--- | :--- |
| `TNS_ADMIN` | *(자동 탐색)* | `tnsnames.ora` 파일이 위치한 디렉토리 경로 지정 |
| `FLIPSY_DOCKER_CONTAINER` | `oracle23ai` | 오라클이 구동 중인 Docker 컨테이너 이름 |
| `FLIPSY_USE_LOCAL_SQLPLUS` | `0` | `1` 설정 시 Docker 대신 로컬 시스템의 `sqlplus` CLI를 직접 호출 |

---

## 8. 빌드 및 실행

### 필수 요구사항
* Rust 1.75+ (Cargo)
* (선택) Oracle Database Docker 컨테이너 또는 로컬 Oracle Client (`sqlplus`)

### 빌드 명령어
```bash
# 개발 모드 실행
cargo run

# 최적화 릴리스 빌드 (LTO, Strip, Panic Abort 적용)
cargo build --release
```

최적화 빌드 결과물은 `target/release/flipsy.exe` (약 5MB 단일 실행 파일)에 생성됩니다.
