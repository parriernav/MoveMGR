# 구현 모델 인계서

## 범위

이 저장소에는 설계 문서·아이콘·화면 목업만 있다. 실제 앱, Rust 엔진, 빌드 스크립트, 테스트는 구현해야 한다. 첫 버전은 0.0.1이며 요구 기능을 임의로 후속 버전으로 미루지 않는다. DESIGN.md가 제품 정책, CONTRACTS.md가 데이터 계약의 기준이다. 충돌을 발견하면 일관되게 수정하고 변경 근거를 기록한다.

## 추천 프로젝트 구조

```text
movemgr/
  README.md
  docs/                       # 현재 설계, 구현 시 갱신
  design/movemgr-icon.svg
  version.json                # 구현할 버전 기준 파일
  package.json
  pnpm-lock.yaml
  .gitattributes
  .gitignore
  src/
    App.svelte
    lib/components/           # 공통 button/dialog/path/extension controls
    lib/features/projects/    # 목록, 초안 편집, 선택 상태
    lib/features/rules/       # 단계형 규칙 편집 및 예시 UI
    lib/features/runs/        # 미리보기, 진행, 기록, 복구 UI
    lib/features/settings/    # 가져오기·경로 재연결
    lib/ipc/                  # 생성 타입 및 API wrapper
    lib/styles/tokens.css
  src-tauri/
    Cargo.toml
    Cargo.lock
    tauri.conf.json
    capabilities/
    icons/
    src/commands/
    src/domain/               # 확장자·필터·추출·비교, 순수 함수
    src/application/          # 계획·실행 coordinator
    src/infrastructure/       # 저장·저널·filesystem adapters
    src/platform/windows.rs   # no-clobber, 파일 ID, metadata
    src/platform/macos.rs
    tests/
  tests/fixtures/             # 실행 시 임시 디렉터리에 복제
  scripts/prepare-build.mjs
  scripts/check-version.mjs
  scripts/set-version.mjs
  .github/workflows/verify.yml
  .github/workflows/package.yml
```

## 구현 순서

1. **골격·버전·아이콘:** Tauri/Svelte/Vite 프로젝트, 0.0.1, 버전 단일 소스, OS 타이틀바, 밝게/어둡게 토큰, native 폴더 선택기, 공통 IPC 오류.
2. **순수 규칙 엔진:** 확장자/이름 필터, grapheme·NFC·case folding, 분류값, 폴더 비교. 계약 예시로 단위 테스트한다. UI와 같은 evaluator를 사용한다.
3. **설정·프로젝트 UI:** CRUD·복제·정렬, 선택 영속화, 단계형 편집, 저장 revision, 자동 저장 실패 표시.
4. **계획 엔진:** 실제 디렉터리 열거, 연결 상태, 소스 교차 검사, 전체 묶음 충돌 예약, 페이지 미리보기. 아직 디스크 이동은 연결하지 않는다.
5. **이동 엔진:** no-clobber platform adapter, 볼륨 간 복사·해시·원본 삭제, journal, 취소·복구, single-instance. 실패 주입 테스트 후 실행 버튼을 연결한다.
6. **가져오기·내보내기:** 교체/추가, schema 검증, 경로 재연결, 원자적 교체, 백업 복구.
7. **양 OS 검증·패키징:** 같은 manifest의 Windows/macOS 빌드, UI·이동 통합 테스트, 아이콘 크기 확인, 산출물과 버전 검증, 서명 설정 안내.

UI 목업의 demo 코드를 실제 앱 로직으로 복사하지 않는다. 목업은 레이아웃·문구·선택 동작의 참고자료다. 실제 데이터 상태는 Rust의 검증/영속화 응답을 기준으로 구현한다.

## 필수 인수 테스트

| ID | 시나리오 | 통과 기준 |
| --- | --- | --- |
| UI-01 | 프로젝트 3개, 체크 0개 | 전체 실행(3)으로 모두 실행 |
| UI-02 | 2개 체크 | 일부 실행(2), 체크한 2개만 목록 순서 실행 |
| UI-03 | 모두 선택 후 전체 해제 | 일부 실행(3) → 전체 실행(3), 즉시 저장 |
| UI-04 | 미체크 프로젝트의 행 실행 | 해당 프로젝트만 실행, 체크 상태 유지 |
| UI-05 | 앱 종료 후 재시작 | 프로젝트, 순서, 체크, 규칙, 테마 복원 |
| UI-06 | 실행 중 두 번째 실행/가져오기 | UI와 backend 모두 거부 |
| UI-07 | 편집 취소 | 기존 규칙 유지, 초안이 실행에 반영되지 않음 |
| UI-08 | 숨김 경로·긴 한글 경로·200% 확대 | 정보·버튼이 겹치지 않고 전체 경로 확인 가능 |
| RULE-01 | jpg/png 다중 선택, JPG 포함 | OR 선별, 다른 확장자는 제외 |
| RULE-02 | only 확장자 0개 | 전체 파일로 해석하지 않고 저장 오류 |
| RULE-03 | 확장자+시작 문자열 | 둘을 모두 만족하는 파일만 선택 |
| RULE-04 | ABC_001.pdf, 앞 3글자 | 키 ABC, 정확/시작/양쪽 N 비교 각각 명세대로 |
| RULE-05 | 거래처A_견적서.pdf | '_' 앞이 거래처A, 폴더 매칭에 동일 값 전달 |
| RULE-06 | 구분자 없음·맨 앞·반복·다문자 | missing 정책, 빈 키, 첫 등장 규칙 준수 |
| RULE-07 | N보다 짧은 이름·조합 한글·이모지 | 짧으면 건너뜀, NFC/grapheme 기준으로 일관 |
| RULE-08 | 폴더 0개/1개/2개 일치 | noMatch 정책/정상/모호함 보류 |
| RULE-09 | 직접 넣기 및 고정 하위 폴더 | 무관한 키 추출 실패로 파일이 제외되지 않음 |
| RULE-10 | .gitignore/a.tar.gz/파일./확장자 없음 | 계약대로 stem/extension 파싱 |
| RULE-11 | 양쪽 앞 N 비교 | N 미만 값은 일치로 처리하지 않음 |
| FS-01 | 미리보기 | 타겟 폴더·임시 파일을 포함해 파일시스템 변경 없음 |
| FS-02 | 같은 볼륨 이동 | 내용·이름 보존, 원본 사라짐, 목적지 기존 항목 덮어쓰기 없음 |
| FS-03 | 다른 볼륨 이동 | 복사·타겟 재해시 후 원본 삭제, byte hash 일치 |
| FS-04 | 복사 중 취소/오류/공간 부족 | 원본 보존, 소유 임시 파일만 정리, 오류 상태 정확 |
| FS-05 | 목적지 확정 뒤 원본 삭제 실패 | sourceRetained, 성공 합계 제외, 재실행 중복 방지 |
| FS-06 | 미리보기 후 원본 교체/변경 | fingerprint 불일치 감지, 새 파일 삭제 금지 |
| FS-07 | 실행 직전 타겟 동명 생성 | no-clobber, skip/번호 정책 유지 |
| FS-08 | 소스=타겟/부모·자식/별칭 | 실행 차단, symlink/junction 우회 금지 |
| FS-09 | 프로젝트 소스 중복/연쇄 타겟 | 실행 묶음 차단 |
| FS-10 | 여러 프로젝트가 같은 타겟 이름 생성 | 목록 순서와 충돌 정책에 따라 결정 |
| FS-11 | 재귀·숨김·링크 | 설정대로 선별, 링크 미추적, 폴더 자체 이동 없음 |
| FS-12 | 잘못된 키 경로·예약 이름·타겟 링크 변경 | 루트 외부 쓰기 차단, 원본 유지 |
| FS-13 | 강제 종료를 각 journal 단계에 주입 | 다음 시작에서 상태 복구, 무단 삭제/자동 재개 없음 |
| FS-14 | NTFS/exFAT/APFS 대소문자 충돌 | OS adapter 정책을 실제 볼륨에서 검증 |
| FS-15 | 추가 스트림·resource fork/xattr 파일 | 검증된 보존 또는 명시적 보류, 조용한 손실 없음 |
| CFG-01 | export → import 교체 | 경로·규칙·체크·순서 동일, 공통 설정 복원 |
| CFG-02 | import 추가 | 새 UUID, 목록 뒤 추가, 원본 공통 설정 유지 |
| CFG-03 | 손상 JSON/미래 schema/10MiB 초과 | 기존 설정 그대로, 명확한 오류 |
| CFG-04 | 저장 도중 강제 종료 | 정상본 또는 정상 백업 복구 가능 |
| CFG-05 | Windows 설정을 Mac으로 가져오기 | 규칙 보존, 경로 재연결 전 실행 차단 |
| VER-01 | 최초 빌드 및 무변경 재빌드 | 모두 0.0.1 |
| VER-02 | 동작 코드/디자인/의존성 수정 빌드 | patch만 한 번 증가 |
| VER-03 | 동일 수정본을 Windows/Mac으로 빌드 | 동일 버전·digest, 제목·메타데이터 일치 |
| VER-04 | 실패 재시도/생성 파일/문서만 변경 | 불필요한 patch 재증가 없음 |
| VER-05 | 수동 minor/major 변경 | 사용자 지시값 적용, 자동으로 상위 숫자 변경하지 않음 |
| BUILD-01 | Windows 포터블 폴더 실행 | 제목·아이콘·파일 대화상자·이동 정상 |
| BUILD-02 | macOS arm64/x64 패키지 실행 | 제목·아이콘·키보드·WKWebView·이동 정상 |

파일 테스트는 새 임시 디렉터리만 사용하고 사용자의 실제 작업 폴더는 건드리지 않는다. 서로 다른 볼륨·OS API·권한·잠금은 mock만으로 통과 처리하지 말고 실제 환경에서 검증 결과를 남긴다. macOS에서 동일한 자동 UI driver가 지원되지 않으면 공통 UI의 browser 테스트와 Mac 실제 앱 수동 검증을 분리해 보고한다.

## 구현 완료 보고

- 구현된 기능과 아직 충족하지 못한 인수 기준을 구분한다.
- Windows/macOS 각각 실제 실행 검증 여부, 빌드된 버전, 포터블 산출물 경로를 기록한다.
- 테스트 실행 결과와 실패 주입 복구 결과를 기록한다.
- 의존성 정확한 버전·lockfile·공식 문서와 달라진 점을 기록한다.
- ‘빌드 가능’과 ‘해당 OS에서 실제 빌드·실행 완료’를 구분한다.
- 사용자 요구에 없는 원격 푸시·릴리스 공개·자격증명 설정은 임의로 수행하지 않는다.
